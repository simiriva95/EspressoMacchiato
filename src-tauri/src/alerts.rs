//! Alert rules: opt-in, edge-triggered (fire on the false→true transition,
//! re-arm when the condition clears) and rate-limited to one notification
//! per kind per 30 minutes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::AlertsConfig;
use crate::platform::PowerSnapshot;

const RATE_LIMIT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertKind {
    ChargeReminder,
    LowBattery,
    Overheat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub kind: AlertKind,
    pub title: String,
    pub body: String,
}

#[derive(Default)]
pub struct AlertEngine {
    last_fired: HashMap<AlertKind, Instant>,
    previous: HashMap<AlertKind, bool>,
}

impl AlertEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn evaluate(
        &mut self,
        cfg: &AlertsConfig,
        snapshot: &PowerSnapshot,
        lang: &str,
        now: Instant,
    ) -> Vec<Alert> {
        let mut fired = Vec::new();

        let rules: [(AlertKind, bool, bool); 3] = [
            (
                AlertKind::ChargeReminder,
                cfg.charge_reminder,
                snapshot.on_ac
                    && snapshot
                        .percent
                        .is_some_and(|p| p >= f32::from(cfg.charge_target_percent)),
            ),
            (
                AlertKind::LowBattery,
                cfg.low_battery,
                !snapshot.on_ac
                    && snapshot
                        .percent
                        .is_some_and(|p| p <= f32::from(cfg.low_battery_percent)),
            ),
            (
                AlertKind::Overheat,
                cfg.overheat,
                snapshot
                    .temperature_c
                    .is_some_and(|t| t >= cfg.overheat_celsius),
            ),
        ];

        for (kind, enabled, condition) in rules {
            let was = self.previous.insert(kind, condition).unwrap_or(false);
            if !enabled || !condition || was {
                continue; // disabled, quiet, or still inside the same episode
            }
            let rate_limited = self
                .last_fired
                .get(&kind)
                .is_some_and(|t| now.duration_since(*t) < RATE_LIMIT);
            if rate_limited {
                continue;
            }
            self.last_fired.insert(kind, now);
            fired.push(message(kind, cfg, snapshot, lang));
        }
        fired
    }
}

/// Notification copy in the app language. Two languages, four messages:
/// a match beats dragging an i18n framework into the backend.
fn message(kind: AlertKind, cfg: &AlertsConfig, snapshot: &PowerSnapshot, lang: &str) -> Alert {
    let it = lang == "it";
    let percent = snapshot.percent.map(|p| p.round() as i64).unwrap_or(0);
    let (title, body) = match kind {
        AlertKind::ChargeReminder => (
            if it {
                "Stacca il caricatore".to_string()
            } else {
                "Unplug the charger".to_string()
            },
            if it {
                format!(
                    "Batteria al {percent}%: oltre il tuo obiettivo del {}%.",
                    cfg.charge_target_percent
                )
            } else {
                format!(
                    "Battery at {percent}%: past your {}% target.",
                    cfg.charge_target_percent
                )
            },
        ),
        AlertKind::LowBattery => {
            let hog = snapshot
                .top_energy_processes
                .first()
                .map(|p| p.name.clone());
            let time = snapshot
                .time_to_empty
                .map(|d| format!("{}m", d.as_secs() / 60));
            (
                if it {
                    "Batteria in riserva".to_string()
                } else {
                    "Battery running low".to_string()
                },
                match (it, hog, time) {
                    (true, Some(hog), Some(t)) => {
                        format!("{percent}% — circa {t} rimasti. Il processo più pesante è {hog}.")
                    }
                    (true, Some(hog), None) => {
                        format!("{percent}%. Il processo più pesante è {hog}.")
                    }
                    (true, None, _) => format!("{percent}% rimanente."),
                    (false, Some(hog), Some(t)) => {
                        format!("{percent}% — about {t} left. Heaviest process: {hog}.")
                    }
                    (false, Some(hog), None) => format!("{percent}%. Heaviest process: {hog}."),
                    (false, None, _) => format!("{percent}% remaining."),
                },
            )
        }
        AlertKind::Overheat => {
            let temp = snapshot.temperature_c.unwrap_or(0.0);
            (
                if it {
                    "Batteria calda".to_string()
                } else {
                    "Battery running hot".to_string()
                },
                if it {
                    format!(
                        "{temp:.0} °C, sopra la soglia di {:.0} °C.",
                        cfg.overheat_celsius
                    )
                } else {
                    format!(
                        "{temp:.0} °C, above your {:.0} °C threshold.",
                        cfg.overheat_celsius
                    )
                },
            )
        }
    };
    Alert { kind, title, body }
}

/// Timer-expired notification copy (fired from the engine event sink, not
/// from the sampler: it is an engine event, not a power condition).
pub fn timer_expired_message(lang: &str) -> (String, String) {
    if lang == "it" {
        (
            "Timer scaduto".to_string(),
            "EspressoMacchiato si è spento: il tempo impostato è finito.".to_string(),
        )
    } else {
        (
            "Timer expired".to_string(),
            "EspressoMacchiato turned off: the set duration is over.".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_all_on() -> AlertsConfig {
        AlertsConfig {
            charge_reminder: true,
            charge_target_percent: 80,
            low_battery: true,
            low_battery_percent: 15,
            overheat: true,
            overheat_celsius: 45.0,
            timer_expired: true,
            calibration_reminder: false,
        }
    }

    fn snap(on_ac: bool, percent: f32, temp: f32) -> PowerSnapshot {
        PowerSnapshot {
            on_ac,
            percent: Some(percent),
            temperature_c: Some(temp),
            ..Default::default()
        }
    }

    #[test]
    fn disabled_rules_never_fire() {
        let mut engine = AlertEngine::new();
        let cfg = AlertsConfig::default(); // everything opt-in → off
        let fired = engine.evaluate(&cfg, &snap(true, 100.0, 90.0), "en", Instant::now());
        assert!(fired.is_empty());
    }

    #[test]
    fn charge_reminder_fires_once_per_episode() {
        let mut engine = AlertEngine::new();
        let cfg = cfg_all_on();
        let now = Instant::now();

        let fired = engine.evaluate(&cfg, &snap(true, 85.0, 30.0), "en", now);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, AlertKind::ChargeReminder);

        // Still charging above target: same episode, no repeat.
        let fired = engine.evaluate(&cfg, &snap(true, 90.0, 30.0), "en", now);
        assert!(fired.is_empty());

        // Unplugged (condition clears), replugged above target much later:
        // re-armed and past the rate limit → fires again.
        engine.evaluate(&cfg, &snap(false, 90.0, 30.0), "en", now);
        let later = now + Duration::from_secs(31 * 60);
        let fired = engine.evaluate(&cfg, &snap(true, 90.0, 30.0), "en", later);
        assert_eq!(fired.len(), 1);
    }

    #[test]
    fn rate_limit_suppresses_flapping() {
        let mut engine = AlertEngine::new();
        let cfg = cfg_all_on();
        let now = Instant::now();

        assert_eq!(
            engine
                .evaluate(&cfg, &snap(false, 10.0, 30.0), "en", now)
                .len(),
            1
        );
        // Condition clears and comes back 5 minutes later: rate limited.
        engine.evaluate(&cfg, &snap(true, 30.0, 30.0), "en", now);
        let soon = now + Duration::from_secs(5 * 60);
        assert!(engine
            .evaluate(&cfg, &snap(false, 10.0, 30.0), "en", soon)
            .is_empty());
        // But after 30 minutes it may fire again.
        engine.evaluate(&cfg, &snap(true, 30.0, 30.0), "en", soon);
        let later = now + Duration::from_secs(31 * 60);
        assert_eq!(
            engine
                .evaluate(&cfg, &snap(false, 10.0, 30.0), "en", later)
                .len(),
            1
        );
    }

    #[test]
    fn overheat_uses_threshold_and_language() {
        let mut engine = AlertEngine::new();
        let cfg = cfg_all_on();
        let now = Instant::now();
        assert!(engine
            .evaluate(&cfg, &snap(true, 50.0, 44.9), "it", now)
            .is_empty());
        let fired = engine.evaluate(&cfg, &snap(true, 50.0, 45.1), "it", now);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].title, "Batteria calda");
    }

    #[test]
    fn low_battery_names_the_heaviest_process() {
        let mut engine = AlertEngine::new();
        let cfg = cfg_all_on();
        let mut s = snap(false, 12.0, 30.0);
        s.top_energy_processes = vec![crate::platform::ProcessUsage {
            name: "chrome".into(),
            cpu_percent: 87.0,
        }];
        s.time_to_empty = Some(Duration::from_secs(40 * 60));
        let fired = engine.evaluate(&cfg, &s, "en", Instant::now());
        assert!(fired[0].body.contains("chrome"));
        assert!(fired[0].body.contains("40m"));
    }

    #[test]
    fn missing_metrics_keep_rules_quiet() {
        let mut engine = AlertEngine::new();
        let cfg = cfg_all_on();
        let empty = PowerSnapshot::default(); // percent/temp all None
        assert!(engine
            .evaluate(&cfg, &empty, "en", Instant::now())
            .is_empty());
    }
}
