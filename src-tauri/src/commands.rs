//! #[tauri::command] endpoints exposed to the frontend.

use tauri::State;
use tauri_plugin_autostart::ManagerExt;

use crate::config::{defaults, Settings};
use crate::core::engine::clamp_interval_secs;
use crate::core::events::{PokeReport, StatusSnapshot};
use crate::core::state::ActivationReason;
use crate::platform::Degradation;
use crate::AppState;

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusSnapshot, String> {
    state.engine.status().await
}

#[tauri::command]
pub async fn set_active(
    state: State<'_, AppState>,
    on: bool,
    duration_secs: Option<u64>,
) -> Result<(), String> {
    state
        .engine
        .set_active(on, ActivationReason::Manual, duration_secs);
    Ok(())
}

#[tauri::command]
pub async fn toggle(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.toggle();
    Ok(())
}

#[tauri::command]
pub async fn poke_now(state: State<'_, AppState>) -> Result<PokeReport, String> {
    state.engine.poke_now().await
}

#[tauri::command]
pub async fn get_idle_seconds(state: State<'_, AppState>) -> Result<f64, String> {
    state.engine.idle_seconds().await
}

/// Live permission checklist for the onboarding / settings UI.
#[tauri::command]
pub fn get_permission_status(state: State<'_, AppState>) -> Vec<Degradation> {
    (state.preflight)()
}

/// macOS: system Accessibility prompt (registers the app in the Privacy
/// list). Returns whether the process is trusted right now.
#[tauri::command]
pub fn request_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::permissions::request_accessibility()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Unix seconds when the current (or imminent) meeting ends, for the
/// "until end of meeting" duration. Ok(None) = calendar readable but no
/// relevant event; Err = permission denied/unavailable. Sync on purpose:
/// it may block on the system permission dialog, and Tauri runs sync
/// commands on a worker thread.
#[tauri::command]
pub fn get_next_meeting_end() -> Result<Option<i64>, String> {
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::calendar::next_meeting_end()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("not available on this platform".into())
    }
}

/// Deep link to the OS panel where the missing permission is granted.
#[tauri::command]
pub fn open_permission_settings(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                None::<String>,
            )
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("no settings panel to open on this platform".into())
    }
}

/// Bring the settings window to the front (used by the popover).
#[tauri::command]
pub fn open_settings_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

/// Latest full power snapshot (None until the first sample lands).
#[tauri::command]
pub fn get_power(hub: State<'_, crate::PowerHub>) -> Option<crate::platform::PowerSnapshot> {
    hub.latest.lock().unwrap().clone()
}

/// In-memory history for the sparklines (2 h @ 10 s, never persisted).
#[tauri::command]
pub fn get_power_history(hub: State<'_, crate::PowerHub>) -> Vec<crate::PowerSample> {
    hub.history.lock().unwrap().iter().cloned().collect()
}

#[derive(serde::Serialize)]
pub struct StatsReport {
    pub week_active_secs: u64,
    pub week_pokes: u64,
    pub shots_today: u32,
    pub health_now: Option<f32>,
    pub health_month_ago: Option<f32>,
}

/// Weekly aggregates for the report card and the espresso-shot chip.
#[tauri::command]
pub fn get_stats(
    hub: State<'_, crate::StatsHub>,
    power: State<'_, crate::PowerHub>,
) -> StatsReport {
    let stats = hub.stats.lock().unwrap();
    StatsReport {
        week_active_secs: stats.week_active_secs,
        week_pokes: stats.week_pokes,
        shots_today: stats.shots_today,
        health_now: power
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.health_percent),
        health_month_ago: stats.health_a_month_ago(),
    }
}

/// Single write path for all settings: sanitize, apply to the engine and
/// the OS (hotkey, autostart), persist (debounced). Returns the sanitized
/// settings so the UI reflects clamping.
#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let mut new = settings;
    new.schema_version = defaults::SCHEMA_VERSION;
    new.interval_secs = clamp_interval_secs(new.interval_secs);
    new.menu_bar_metrics
        .retain(|m| matches!(m.as_str(), "countdown" | "battery" | "watts"));
    new.menu_bar_metrics.truncate(2);
    new.alerts.charge_target_percent = new.alerts.charge_target_percent.clamp(1, 100);
    new.alerts.low_battery_percent = new.alerts.low_battery_percent.clamp(1, 100);
    if !matches!(new.menu_bar_ring.as_str(), "off" | "timer" | "battery") {
        new.menu_bar_ring = "timer".into();
    }
    // Accent must be a plain hex color (it lands in a CSS custom property).
    let accent_ok = new.accent.len() == 7
        && new.accent.starts_with('#')
        && new.accent[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !accent_ok {
        new.accent = "#e8a54c".into();
    }

    let old = state.settings.lock().unwrap().clone();

    // Hotkey first: it is the only fallible step, and failing early leaves
    // everything untouched.
    if new.hotkey != old.hotkey {
        crate::register_hotkey(&app, &new.hotkey)
            .map_err(|e| format!("hotkey '{}' not usable: {e}", new.hotkey))?;
    }

    state.engine.apply_config(
        new.interval_secs,
        new.schedule.clone(),
        new.conditions.clone(),
    );
    if new.strategy != old.strategy {
        state.engine.set_strategy(new.strategy);
    }
    if new.autostart != old.autostart {
        let autolaunch = app.autolaunch();
        let result = if new.autostart {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(e) = result {
            tracing::warn!("autostart change failed: {e}");
        }
    }

    if new.hud_enabled != old.hud_enabled {
        use tauri::Manager;
        if let Some(hud) = app.get_webview_window("hud") {
            if new.hud_enabled {
                let _ = hud.show();
            } else {
                let _ = hud.hide();
            }
        }
    }

    *state.settings.lock().unwrap() = new.clone();
    let _ = state.saver.send(new.clone());
    Ok(new)
}
