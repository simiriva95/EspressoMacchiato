//! Engine state machine types. Pure data, no I/O.

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationReason {
    Manual,
    Hotkey,
    Schedule,
    Autostart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    NeverStarted,
    UserToggle,
    TimerExpired,
    #[allow(dead_code)] // set by the scheduler (M2)
    ScheduleClosed,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuspendReason {
    OnBattery,
    ProcessNotRunning,
    ScreenLocked,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineState {
    Off {
        reason: StopReason,
    },
    Active {
        reason: ActivationReason,
        deadline: Option<Instant>,
    },
    /// A gate is unsatisfied: inhibitor released, pokes paused. Comes back
    /// to Active on its own when the gate is satisfied again.
    Suspended {
        reason: SuspendReason,
        resume_as: ActivationReason,
        deadline: Option<Instant>,
    },
    /// Inhibitor may be fine but poking is unavailable (missing permission).
    /// First-class state: visible in tray, actionable notification.
    Degraded {
        reason: ActivationReason,
        deadline: Option<Instant>,
        detail: String,
    },
}

impl EngineState {
    /// Engine is "on" from the user's point of view (inhibitor held or wanted).
    pub fn is_on(&self) -> bool {
        !matches!(self, EngineState::Off { .. })
    }

    /// Pokes should be attempted in this state.
    pub fn should_poke(&self) -> bool {
        matches!(
            self,
            EngineState::Active { .. } | EngineState::Degraded { .. }
        )
    }

    pub fn deadline(&self) -> Option<Instant> {
        match self {
            EngineState::Active { deadline, .. }
            | EngineState::Suspended { deadline, .. }
            | EngineState::Degraded { deadline, .. } => *deadline,
            EngineState::Off { .. } => None,
        }
    }

    pub fn activation_reason(&self) -> Option<ActivationReason> {
        match self {
            EngineState::Active { reason, .. } | EngineState::Degraded { reason, .. } => {
                Some(*reason)
            }
            EngineState::Suspended { resume_as, .. } => Some(*resume_as),
            EngineState::Off { .. } => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            EngineState::Off { .. } => "off",
            EngineState::Active { .. } => "active",
            EngineState::Suspended { .. } => "suspended",
            EngineState::Degraded { .. } => "degraded",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_is_not_on_and_never_pokes() {
        let s = EngineState::Off {
            reason: StopReason::NeverStarted,
        };
        assert!(!s.is_on());
        assert!(!s.should_poke());
        assert_eq!(s.deadline(), None);
        assert_eq!(s.activation_reason(), None);
    }

    #[test]
    fn active_pokes() {
        let s = EngineState::Active {
            reason: ActivationReason::Manual,
            deadline: None,
        };
        assert!(s.is_on());
        assert!(s.should_poke());
        assert_eq!(s.activation_reason(), Some(ActivationReason::Manual));
    }

    #[test]
    fn suspended_is_on_but_does_not_poke() {
        let s = EngineState::Suspended {
            reason: SuspendReason::OnBattery,
            resume_as: ActivationReason::Schedule,
            deadline: None,
        };
        assert!(s.is_on());
        assert!(!s.should_poke());
        assert_eq!(s.activation_reason(), Some(ActivationReason::Schedule));
    }

    #[test]
    fn degraded_still_attempts_pokes() {
        let s = EngineState::Degraded {
            reason: ActivationReason::Manual,
            deadline: None,
            detail: "no accessibility permission".into(),
        };
        assert!(s.is_on());
        assert!(s.should_poke());
    }
}
