pub mod activity_uinput;
pub mod activity_xtest;
pub mod conditions;
pub mod idle;
pub mod inhibitor;
pub mod power;
pub mod session;

use std::sync::Arc;

use super::{
    ActivitySimulator, ActivityStrategy, Degradation, DegradationKind, Platform, PlatformError,
};

/// Picks uinput first (X11 + Wayland), XTest second (X11 only). The backend
/// is (re)initialized lazily on each poke attempt, so granting /dev/uinput
/// access at runtime recovers from Degraded without a restart.
struct LinuxActivitySimulator {
    backend: Option<Box<dyn ActivitySimulator>>,
    strategy: ActivityStrategy,
}

impl LinuxActivitySimulator {
    fn new() -> Self {
        let mut sim = Self {
            backend: None,
            strategy: ActivityStrategy::ZeroMouseMove,
        };
        let _ = sim.ensure_backend();
        sim
    }

    fn ensure_backend(&mut self) -> Result<&mut Box<dyn ActivitySimulator>, PlatformError> {
        if self.backend.is_none() {
            let backend: Box<dyn ActivitySimulator> = match activity_uinput::UinputSimulator::new()
            {
                Ok(s) => Box::new(s),
                Err(uinput_err) => match activity_xtest::XTestSimulator::new() {
                    Ok(s) => {
                        tracing::info!("uinput unavailable ({uinput_err}), using XTest");
                        Box::new(s)
                    }
                    Err(xtest_err) => {
                        return Err(PlatformError::PermissionDenied(format!(
                            "no activity backend: uinput: {uinput_err}; XTest: {xtest_err}"
                        )));
                    }
                },
            };
            self.backend = Some(backend);
            if let Some(b) = self.backend.as_mut() {
                // Best effort: keep the chosen strategy across backend swaps.
                let _ = b.set_strategy(self.strategy);
            }
        }
        Ok(self.backend.as_mut().unwrap())
    }
}

impl ActivitySimulator for LinuxActivitySimulator {
    fn strategy(&self) -> ActivityStrategy {
        self.strategy
    }

    fn available_strategies(&self) -> Vec<ActivityStrategy> {
        match &self.backend {
            Some(b) => b.available_strategies(),
            None => vec![ActivityStrategy::ZeroMouseMove],
        }
    }

    fn set_strategy(&mut self, s: ActivityStrategy) -> Result<(), PlatformError> {
        if let Some(b) = self.backend.as_mut() {
            b.set_strategy(s)?;
        }
        self.strategy = s;
        Ok(())
    }

    fn poke(&mut self) -> Result<(), PlatformError> {
        match self.ensure_backend() {
            Ok(backend) => backend.poke(),
            Err(e) => Err(e),
        }
    }
}

fn uinput_writable() -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

pub fn platform() -> Platform {
    let info = session::detect();
    tracing::info!(
        "linux session: {:?}, desktop: {}",
        info.session_type,
        info.desktop
    );

    let idle: Box<dyn super::IdleReader> = match idle::LinuxIdleReader::detect() {
        Ok(reader) => Box::new(reader),
        Err(e) => Box::new(idle::UnavailableIdleReader(e.to_string())),
    };

    Platform {
        inhibitor: Box::new(inhibitor::LinuxInhibitor::default()),
        simulator: Box::new(LinuxActivitySimulator::new()),
        idle,
        conditions: Box::new(conditions::LinuxConditionProbe::new()),
        power: Box::new(power::LinuxPowerMonitor::new()),
        preflight: Arc::new(|| {
            let mut degradations = Vec::new();
            // Only a real blocker degrades: uinput missing while XTest can
            // still cover an X11 session is logged, not raised.
            if !uinput_writable() && !session::x11_reachable() {
                let info = session::detect();
                degradations.push(Degradation {
                    what: DegradationKind::PokeUnavailable,
                    detail: format!(
                        "/dev/uinput is not writable and no X server is reachable ({:?} session, {}): no way to inject the synthetic activity that resets the idle counter.",
                        info.session_type, info.desktop
                    ),
                    help: Some(activity_uinput::UINPUT_HELP.into()),
                });
            }
            degradations
        }),
    }
}
