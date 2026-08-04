//! Pure gate logic: given the config and the observed facts, should the
//! engine be suspended, and why? All gates are opt-in and default off
//! (except pause_when_input_recent, which lives in the poke cycle).

use super::state::SuspendReason;
use crate::config::ConditionsConfig;

/// Facts collected by the platform probe right before evaluation.
/// `None` = not observable on this system → the gate does not fire
/// (a gate must never suspend on missing data, except the battery
/// threshold below which is conservative by design).
#[derive(Debug, Clone, Copy, Default)]
pub struct GateFacts {
    pub on_ac: Option<bool>,
    pub battery_percent: Option<f32>,
    pub screen_locked: Option<bool>,
    /// Only collected when the process gate is enabled.
    pub required_process_running: Option<bool>,
}

pub fn required_suspension(cfg: &ConditionsConfig, facts: &GateFacts) -> Option<SuspendReason> {
    if cfg.only_on_ac && facts.on_ac == Some(false) {
        let fire = match cfg.min_battery_percent {
            None => true,
            // Threshold set: suspend only below it. Unreadable percent →
            // suspend anyway (conservative: the user asked to protect the
            // battery).
            Some(threshold) => facts
                .battery_percent
                .is_none_or(|p| p < f32::from(threshold)),
        };
        if fire {
            return Some(SuspendReason::OnBattery);
        }
    }
    if cfg.only_when_process_running && facts.required_process_running == Some(false) {
        return Some(SuspendReason::ProcessNotRunning);
    }
    if cfg.pause_when_screen_locked && facts.screen_locked == Some(true) {
        return Some(SuspendReason::ScreenLocked);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ConditionsConfig {
        ConditionsConfig::default()
    }

    #[test]
    fn all_gates_off_never_suspends() {
        let facts = GateFacts {
            on_ac: Some(false),
            screen_locked: Some(true),
            required_process_running: Some(false),
            battery_percent: Some(1.0),
        };
        assert_eq!(required_suspension(&cfg(), &facts), None);
    }

    #[test]
    fn on_battery_gate() {
        let mut c = cfg();
        c.only_on_ac = true;
        let mut facts = GateFacts {
            on_ac: Some(false),
            ..Default::default()
        };
        assert_eq!(
            required_suspension(&c, &facts),
            Some(SuspendReason::OnBattery)
        );
        facts.on_ac = Some(true);
        assert_eq!(required_suspension(&c, &facts), None);
        // Unknown power state: the gate must not fire on missing data.
        facts.on_ac = None;
        assert_eq!(required_suspension(&c, &facts), None);
    }

    #[test]
    fn battery_threshold() {
        let mut c = cfg();
        c.only_on_ac = true;
        c.min_battery_percent = Some(30);
        let mut facts = GateFacts {
            on_ac: Some(false),
            battery_percent: Some(50.0),
            ..Default::default()
        };
        assert_eq!(required_suspension(&c, &facts), None, "above threshold");
        facts.battery_percent = Some(20.0);
        assert_eq!(
            required_suspension(&c, &facts),
            Some(SuspendReason::OnBattery)
        );
        facts.battery_percent = None;
        assert_eq!(
            required_suspension(&c, &facts),
            Some(SuspendReason::OnBattery),
            "unreadable percent is conservative"
        );
    }

    #[test]
    fn process_gate() {
        let mut c = cfg();
        c.only_when_process_running = true;
        let mut facts = GateFacts {
            required_process_running: Some(false),
            ..Default::default()
        };
        assert_eq!(
            required_suspension(&c, &facts),
            Some(SuspendReason::ProcessNotRunning)
        );
        facts.required_process_running = Some(true);
        assert_eq!(required_suspension(&c, &facts), None);
    }

    #[test]
    fn screen_lock_gate_and_priority() {
        let mut c = cfg();
        c.pause_when_screen_locked = true;
        let facts = GateFacts {
            screen_locked: Some(true),
            ..Default::default()
        };
        assert_eq!(
            required_suspension(&c, &facts),
            Some(SuspendReason::ScreenLocked)
        );
        // Battery gate outranks the lock gate when both fire.
        c.only_on_ac = true;
        let facts = GateFacts {
            on_ac: Some(false),
            screen_locked: Some(true),
            ..Default::default()
        };
        assert_eq!(
            required_suspension(&c, &facts),
            Some(SuspendReason::OnBattery)
        );
    }
}
