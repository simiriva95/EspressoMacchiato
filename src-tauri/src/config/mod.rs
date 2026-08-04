//! Settings persistence: one `settings.json`, atomic writes (temp file +
//! rename), 500 ms debounce, schema versioning with migrations, corrupt
//! files renamed aside instead of crashing.

pub mod defaults;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::platform::ActivityStrategy;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleWindow {
    /// Days of week, 0 = Monday … 6 = Sunday.
    pub days: Vec<u8>,
    /// "HH:MM" local wall clock.
    pub start: String,
    /// "HH:MM" exclusive; if end <= start the window crosses midnight.
    pub end: String,
}

impl Default for ScheduleWindow {
    fn default() -> Self {
        Self {
            days: vec![0, 1, 2, 3, 4],
            start: "09:00".into(),
            end: "18:00".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleConfig {
    pub enabled: bool,
    pub windows: Vec<ScheduleWindow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConditionsConfig {
    pub only_on_ac: bool,
    /// With `only_on_ac`: suspend only below this % (None = any battery).
    pub min_battery_percent: Option<u8>,
    pub only_when_process_running: bool,
    pub process_names: Vec<String>,
    pub pause_when_screen_locked: bool,
    pub pause_when_input_recent: bool,
}

impl Default for ConditionsConfig {
    fn default() -> Self {
        Self {
            only_on_ac: false,
            min_battery_percent: None,
            only_when_process_running: false,
            process_names: defaults::suggested_process_names(),
            pause_when_screen_locked: false,
            pause_when_input_recent: defaults::PAUSE_WHEN_INPUT_RECENT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub interval_secs: u64,
    pub strategy: ActivityStrategy,
    pub schedule: ScheduleConfig,
    pub conditions: ConditionsConfig,
    pub autostart: bool,
    pub activate_on_start: bool,
    pub hotkey: String,
    /// "HH:MM" used by the "until end of day" duration option.
    pub end_of_day: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: defaults::SCHEMA_VERSION,
            interval_secs: defaults::INTERVAL_SECS,
            strategy: ActivityStrategy::ZeroMouseMove,
            schedule: ScheduleConfig::default(),
            conditions: ConditionsConfig::default(),
            autostart: false,
            activate_on_start: false,
            hotkey: defaults::HOTKEY.into(),
            end_of_day: defaults::END_OF_DAY.into(),
        }
    }
}

/// Platform config dir per spec: `~/Library/Application Support/
/// app.espressomacchiato/` on macOS, `$XDG_CONFIG_HOME/espresso-macchiato/`
/// on Linux.
pub fn settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let dirs = directories::ProjectDirs::from("app", "", "espressomacchiato")?;
    #[cfg(not(target_os = "macos"))]
    let dirs = directories::ProjectDirs::from("app", "", "espresso-macchiato")?;
    Some(dirs.config_dir().join("settings.json"))
}

#[derive(Debug, PartialEq)]
pub enum LoadOutcome {
    Loaded,
    Defaults,
    /// The file was corrupt: renamed to `.corrupt-<timestamp>`, defaults
    /// applied. The caller should notify once.
    CorruptRecovered(PathBuf),
}

pub fn load(path: &Path) -> (Settings, LoadOutcome) {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return (Settings::default(), LoadOutcome::Defaults),
    };
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) => (migrate(value), LoadOutcome::Loaded),
        Err(e) => {
            tracing::warn!("settings.json corrupt ({e}), starting from defaults");
            let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let quarantine = path.with_extension(format!("corrupt-{ts}"));
            let _ = std::fs::rename(path, &quarantine);
            (
                Settings::default(),
                LoadOutcome::CorruptRecovered(quarantine),
            )
        }
    }
}

/// Sequential migrations from any older schema to the current one.
/// `#[serde(default)]` absorbs added fields; this hook exists for renames
/// and semantic changes.
fn migrate(mut value: serde_json::Value) -> Settings {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    #[allow(clippy::single_match)] // more steps will stack here over time
    match version {
        0 => {
            // v0 → v1: no shipped v0 files exist; just stamp the version.
            value["schema_version"] = defaults::SCHEMA_VERSION.into();
        }
        _ => {}
    }
    match serde_json::from_value::<Settings>(value) {
        Ok(mut s) => {
            s.schema_version = defaults::SCHEMA_VERSION;
            s.interval_secs = crate::core::engine::clamp_interval_secs(s.interval_secs);
            s
        }
        Err(e) => {
            tracing::warn!("settings.json has invalid fields ({e}), using defaults");
            Settings::default()
        }
    }
}

/// Atomic write: temp file in the same directory, then rename.
pub fn save_atomic(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let dir = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(".settings.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(&tmp, path)
}

/// Debounced saver: keeps only the latest snapshot, writes 500 ms after the
/// last change. Returns a sender; the task lives on the given runtime.
pub fn spawn_saver(path: PathBuf) -> tokio::sync::mpsc::UnboundedSender<Settings> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Settings>();
    tauri::async_runtime::spawn(async move {
        while let Some(mut latest) = rx.recv().await {
            // Debounce: absorb further updates for 500 ms.
            loop {
                tokio::select! {
                    more = rx.recv() => match more {
                        Some(s) => latest = s,
                        None => break,
                    },
                    _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => break,
                }
            }
            if let Err(e) = save_atomic(&path, &latest) {
                tracing::error!("failed to save settings: {e}");
            }
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "espresso-config-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip_save_load() {
        let path = tmp_dir().join("settings.json");
        let s = Settings {
            interval_secs: 45,
            schedule: ScheduleConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        save_atomic(&path, &s).unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(loaded, s);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (loaded, outcome) = load(&tmp_dir().join("nope.json"));
        assert_eq!(outcome, LoadOutcome::Defaults);
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn corrupt_file_is_quarantined_and_defaults_apply() {
        let dir = tmp_dir();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{not json!!").unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(loaded, Settings::default());
        let LoadOutcome::CorruptRecovered(quarantine) = outcome else {
            panic!("expected CorruptRecovered, got {outcome:?}");
        };
        assert!(quarantine.exists(), "corrupt file kept for inspection");
        assert!(!path.exists(), "original slot is free again");
    }

    #[test]
    fn unknown_and_missing_fields_migrate_to_current_schema() {
        let dir = tmp_dir();
        let path = dir.join("settings.json");
        // A hypothetical v0 file: no schema_version, unknown field, missing
        // most fields, out-of-range interval.
        std::fs::write(
            &path,
            r#"{ "interval_secs": 5, "some_removed_field": true }"#,
        )
        .unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(loaded.schema_version, defaults::SCHEMA_VERSION);
        assert_eq!(loaded.interval_secs, 10, "interval clamped on load");
        assert_eq!(loaded.hotkey, defaults::HOTKEY);
    }

    #[test]
    fn atomic_write_leaves_no_temp_file() {
        let path = tmp_dir().join("settings.json");
        save_atomic(&path, &Settings::default()).unwrap();
        assert!(path.exists());
        assert!(!path.parent().unwrap().join(".settings.json.tmp").exists());
    }
}
