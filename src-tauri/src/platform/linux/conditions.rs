//! Condition probes: /sys/class/power_supply for power facts, logind's
//! LockedHint for the screen lock, sysinfo for the process gate.

use std::path::Path;

use zbus::blocking::{Connection, Proxy};

use crate::platform::{ConditionProbe, ProcessMatcher};

fn read_sys(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Some systems expose several supplies; AC wins if any "Mains" is online.
fn scan_power_supply() -> (Option<bool>, Option<f32>) {
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
        return (None, None);
    };
    let mut on_ac: Option<bool> = None;
    let mut percent: Option<f32> = None;
    for entry in entries.flatten() {
        let dir = entry.path();
        match read_sys(&dir.join("type")).as_deref() {
            Some("Mains") => {
                let online = read_sys(&dir.join("online")).as_deref() == Some("1");
                on_ac = Some(on_ac.unwrap_or(false) || online);
            }
            Some("Battery") if percent.is_none() => {
                percent = read_sys(&dir.join("capacity")).and_then(|v| v.parse().ok());
            }
            _ => {}
        }
    }
    // Desktop with no battery and no Mains entry: unknown, gates stay quiet.
    (on_ac, percent)
}

fn logind_locked_hint() -> Option<bool> {
    let conn = Connection::system().ok()?;
    let proxy = Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1/session/auto",
        "org.freedesktop.login1.Session",
    )
    .ok()?;
    proxy.get_property("LockedHint").ok()
}

pub struct LinuxConditionProbe {
    processes: ProcessMatcher,
}

impl LinuxConditionProbe {
    pub fn new() -> Self {
        Self {
            processes: ProcessMatcher::new(),
        }
    }
}

impl Default for LinuxConditionProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl ConditionProbe for LinuxConditionProbe {
    fn on_ac(&self) -> Option<bool> {
        scan_power_supply().0
    }

    fn battery_percent(&self) -> Option<f32> {
        scan_power_supply().1
    }

    fn screen_locked(&self) -> Option<bool> {
        logind_locked_hint()
    }

    fn any_process_running(&self, names: &[String]) -> bool {
        self.processes.any_running(names)
    }

    /// PulseAudio/PipeWire: any active recording stream = in a call.
    fn mic_in_use(&self) -> Option<bool> {
        let out = std::process::Command::new("pactl")
            .args(["list", "short", "source-outputs"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
    }
}
