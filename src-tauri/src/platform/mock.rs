//! Mock platform for tests and CI.
//!
//! Counts every call and can inject failures so the engine's `Degraded`
//! paths are exercised without real hardware.

use std::sync::{Arc, Mutex};

use super::{
    ActivitySimulator, ActivityStrategy, ConditionProbe, Degradation, IdleReader, InhibitOptions,
    NullPowerMonitor, Platform, PlatformError, SleepInhibitor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // constructed only from #[cfg(test)] code
pub enum MockFailure {
    PermissionDenied,
    Other,
}

pub struct MockState {
    pub acquires: u32,
    pub releases: u32,
    pub inhibited: bool,
    pub pokes: u32,
    pub idle_secs: f64,
    pub strategy: ActivityStrategy,
    pub acquire_fail: bool,
    pub poke_fail: Option<MockFailure>,
    pub idle_fail: bool,
    pub degradations: Vec<Degradation>,
    // Condition probe facts, all controllable from tests.
    pub on_ac: Option<bool>,
    pub battery_percent: Option<f32>,
    pub screen_locked: Option<bool>,
    pub process_running: bool,
    pub mic_in_use: Option<bool>,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            acquires: 0,
            releases: 0,
            inhibited: false,
            pokes: 0,
            // Idle high by default so `pause_when_input_recent` never skips
            // unless a test lowers it on purpose.
            idle_secs: 9999.0,
            strategy: ActivityStrategy::ZeroMouseMove,
            acquire_fail: false,
            poke_fail: None,
            idle_fail: false,
            degradations: Vec::new(),
            on_ac: Some(true),
            battery_percent: Some(80.0),
            screen_locked: Some(false),
            process_running: true,
            mic_in_use: Some(false),
        }
    }
}

pub type SharedMock = Arc<Mutex<MockState>>;

pub struct MockInhibitor(pub SharedMock);

impl SleepInhibitor for MockInhibitor {
    fn acquire(&mut self, _opts: InhibitOptions) -> Result<(), PlatformError> {
        let mut s = self.0.lock().unwrap();
        if s.acquire_fail {
            return Err(PlatformError::Other("mock acquire failure".into()));
        }
        s.acquires += 1;
        s.inhibited = true;
        Ok(())
    }

    fn release(&mut self) -> Result<(), PlatformError> {
        let mut s = self.0.lock().unwrap();
        if s.inhibited {
            s.releases += 1;
            s.inhibited = false;
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.0.lock().unwrap().inhibited
    }
}

impl Drop for MockInhibitor {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

pub struct MockSimulator(pub SharedMock);

impl ActivitySimulator for MockSimulator {
    fn strategy(&self) -> ActivityStrategy {
        self.0.lock().unwrap().strategy
    }

    fn available_strategies(&self) -> Vec<ActivityStrategy> {
        vec![
            ActivityStrategy::ZeroMouseMove,
            ActivityStrategy::HarmlessKeyTap,
            ActivityStrategy::NudgeAndReturn,
        ]
    }

    fn set_strategy(&mut self, s: ActivityStrategy) -> Result<(), PlatformError> {
        self.0.lock().unwrap().strategy = s;
        Ok(())
    }

    fn poke(&mut self) -> Result<(), PlatformError> {
        let mut s = self.0.lock().unwrap();
        match s.poke_fail {
            Some(MockFailure::PermissionDenied) => Err(PlatformError::PermissionDenied(
                "mock permission denied".into(),
            )),
            Some(MockFailure::Other) => Err(PlatformError::Other("mock poke failure".into())),
            None => {
                s.pokes += 1;
                Ok(())
            }
        }
    }
}

pub struct MockIdleReader(pub SharedMock);

impl IdleReader for MockIdleReader {
    fn idle_seconds(&self) -> Result<f64, PlatformError> {
        let s = self.0.lock().unwrap();
        if s.idle_fail {
            return Err(PlatformError::Unsupported("mock idle unreadable".into()));
        }
        Ok(s.idle_secs)
    }

    fn source(&self) -> &'static str {
        "mock"
    }
}

pub struct MockConditionProbe(pub SharedMock);

impl ConditionProbe for MockConditionProbe {
    fn on_ac(&self) -> Option<bool> {
        self.0.lock().unwrap().on_ac
    }

    fn battery_percent(&self) -> Option<f32> {
        self.0.lock().unwrap().battery_percent
    }

    fn screen_locked(&self) -> Option<bool> {
        self.0.lock().unwrap().screen_locked
    }

    fn any_process_running(&self, _names: &[String]) -> bool {
        self.0.lock().unwrap().process_running
    }

    fn mic_in_use(&self) -> Option<bool> {
        self.0.lock().unwrap().mic_in_use
    }
}

pub fn mock_platform() -> (Platform, SharedMock) {
    let shared: SharedMock = Arc::new(Mutex::new(MockState::default()));
    let preflight_shared = shared.clone();
    let platform = Platform {
        inhibitor: Box::new(MockInhibitor(shared.clone())),
        simulator: Box::new(MockSimulator(shared.clone())),
        idle: Box::new(MockIdleReader(shared.clone())),
        conditions: Box::new(MockConditionProbe(shared.clone())),
        power: Box::new(NullPowerMonitor),
        preflight: Arc::new(move || preflight_shared.lock().unwrap().degradations.clone()),
    };
    (platform, shared)
}
