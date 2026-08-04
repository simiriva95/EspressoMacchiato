//! Platform abstraction layer.
//!
//! All OS-specific behavior lives behind these traits. Domain logic (the
//! engine) only ever sees this module's types. Real implementations live in
//! `macos/` and `linux/`; `mock.rs` backs tests and CI.

#[cfg(any(test, not(any(target_os = "macos", target_os = "linux"))))]
pub mod mock;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // all variants constructed by the real platform impls (M1)
pub enum PlatformError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("unsupported on this system: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Copy)]
pub struct InhibitOptions {
    /// Also prevent display sleep / screen lock, not just system sleep.
    /// Required for the presence use case: a locked screen drops presence.
    #[allow(dead_code)] // read by the real inhibitors (M1)
    pub keep_display_on: bool,
}

impl Default for InhibitOptions {
    fn default() -> Self {
        Self {
            keep_display_on: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStrategy {
    /// Mouse move with zero delta at the current position. Default where available.
    ZeroMouseMove,
    /// Down+up of a key with no effect (F15 on macOS, KEY_F15 on Linux).
    HarmlessKeyTap,
    /// 1px away and immediately back. Visible fallback, never the default.
    NudgeAndReturn,
}

/// Prevents system sleep, display sleep and screen lock.
/// Implementations MUST keep resources alive (assertion ids, fds, cookies)
/// and release them in Drop.
pub trait SleepInhibitor: Send + Sync {
    fn acquire(&mut self, opts: InhibitOptions) -> Result<(), PlatformError>;
    fn release(&mut self) -> Result<(), PlatformError>;
    fn is_active(&self) -> bool;
}

/// Resets the OS idle counter by injecting synthetic activity.
pub trait ActivitySimulator: Send + Sync {
    fn strategy(&self) -> ActivityStrategy;
    fn available_strategies(&self) -> Vec<ActivityStrategy>;
    fn set_strategy(&mut self, s: ActivityStrategy) -> Result<(), PlatformError>;
    /// A single injection. Must be invisible to the user.
    fn poke(&mut self) -> Result<(), PlatformError>;
}

/// Reads how many seconds the OS believes the user has been idle.
/// This is the source of truth to verify that `poke()` actually works.
pub trait IdleReader: Send + Sync {
    fn idle_seconds(&self) -> Result<f64, PlatformError>;
    fn source(&self) -> &'static str; // e.g. "CGEventSource", "XScreenSaver", "Mutter"
}

/// Observations feeding the opt-in gates (core/conditions.rs). `None`
/// means "not observable on this system" — gates never fire on missing
/// data (the one documented exception is the battery threshold).
pub trait ConditionProbe: Send + Sync {
    fn on_ac(&self) -> Option<bool>;
    fn battery_percent(&self) -> Option<f32>;
    fn screen_locked(&self) -> Option<bool>;
    fn any_process_running(&self, names: &[String]) -> bool;
}

/// Case-insensitive substring match on executable names, shared by the
/// real probes. sysinfo refresh is bounded by the 10 s gate tick.
pub struct ProcessMatcher(std::sync::Mutex<sysinfo::System>);

impl ProcessMatcher {
    pub fn new() -> Self {
        Self(std::sync::Mutex::new(sysinfo::System::new()))
    }

    pub fn any_running(&self, names: &[String]) -> bool {
        let needles: Vec<String> = names
            .iter()
            .map(|n| n.trim().to_lowercase())
            .filter(|n| !n.is_empty())
            .collect();
        if needles.is_empty() {
            return false;
        }
        let mut sys = self.0.lock().unwrap();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes().values().any(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            needles.iter().any(|needle| name.contains(needle))
        })
    }
}

impl Default for ProcessMatcher {
    fn default() -> Self {
        Self::new()
    }
}

pub trait PowerMonitor: Send + Sync {
    #[allow(dead_code)] // wired to the energy panel in M4
    fn snapshot(&self) -> Result<PowerSnapshot, PlatformError>;
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[allow(dead_code)] // energy panel (M4)
pub struct ProcessUsage {
    pub name: String,
    pub cpu_percent: f32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[allow(dead_code)] // energy panel (M4)
pub struct PowerSnapshot {
    pub on_ac: bool,
    pub percent: Option<f32>,
    pub cycle_count: Option<u32>,
    pub design_capacity_mah: Option<u32>,
    pub max_capacity_mah: Option<u32>,
    pub health_percent: Option<f32>,
    pub temperature_c: Option<f32>,
    pub voltage_v: Option<f32>,
    pub amperage_ma: Option<i32>,
    pub watts: Option<f32>,
    pub time_to_empty: Option<Duration>,
    pub time_to_full: Option<Duration>,
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub top_energy_processes: Vec<ProcessUsage>,
}

/// A missing capability that puts the engine in `Degraded` state instead of
/// failing silently. `help` carries the exact user-facing remediation
/// (deep link on macOS, shell commands on Linux).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Degradation {
    pub what: DegradationKind,
    pub detail: String,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradationKind {
    PokeUnavailable,
    InhibitUnavailable,
    IdleUnreadable,
}

pub type PreflightFn = Arc<dyn Fn() -> Vec<Degradation> + Send + Sync>;

/// Everything the engine needs from the OS, bundled.
pub struct Platform {
    pub inhibitor: Box<dyn SleepInhibitor>,
    pub simulator: Box<dyn ActivitySimulator>,
    pub idle: Box<dyn IdleReader>,
    pub conditions: Box<dyn ConditionProbe>,
    #[allow(dead_code)] // energy panel (M4)
    pub power: Box<dyn PowerMonitor>,
    /// Re-checkable capability probe (permissions can change at runtime).
    pub preflight: PreflightFn,
}

/// Power monitoring lands in M4; until then every platform reports
/// "not available" instead of fake zeros.
pub struct NullPowerMonitor;

impl PowerMonitor for NullPowerMonitor {
    fn snapshot(&self) -> Result<PowerSnapshot, PlatformError> {
        Err(PlatformError::Unsupported(
            "power monitoring not implemented yet (M4)".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
pub fn current_platform() -> Platform {
    macos::platform()
}

#[cfg(target_os = "linux")]
pub fn current_platform() -> Platform {
    linux::platform()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn current_platform() -> Platform {
    mock::mock_platform().0
}
