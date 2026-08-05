//! Lightweight local stats: weekly presence time, espresso shots, battery
//! health trend. One small stats.json next to settings.json — aggregates
//! only, never raw metric samples, never leaves the machine.

use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// Monday of the current tracking week (ISO date).
    pub week_start: String,
    pub week_active_secs: u64,
    pub week_pokes: u64,
    /// Engine poke counter last seen (session-scoped, resets on restart).
    pub last_seen_pokes: u64,
    /// Espresso shots (completed 25-minute runs) today.
    pub shots_date: String,
    pub shots_today: u32,
    /// One battery-health point per month ("YYYY-MM", percent).
    pub health_by_month: Vec<(String, f32)>,
    pub last_report_week: String,
    pub last_calibration_month: String,
}

/// A completed timed run of exactly this length counts as one espresso shot.
pub const SHOT_SECS: u64 = 25 * 60;

impl Stats {
    /// Reset week/day buckets when the calendar moved on.
    pub fn rollover(&mut self, today: NaiveDate) {
        let monday = today - chrono::Days::new(u64::from(today.weekday().num_days_from_monday()));
        let monday = monday.to_string();
        if self.week_start != monday {
            self.week_start = monday;
            self.week_active_secs = 0;
            self.week_pokes = 0;
        }
        let today = today.to_string();
        if self.shots_date != today {
            self.shots_date = today;
            self.shots_today = 0;
        }
    }

    /// Accumulate the session-scoped engine poke counter, surviving app
    /// restarts (counter going backwards = new session).
    pub fn track_pokes(&mut self, current_session_count: u64) {
        if current_session_count < self.last_seen_pokes {
            self.last_seen_pokes = 0;
        }
        self.week_pokes += current_session_count - self.last_seen_pokes;
        self.last_seen_pokes = current_session_count;
    }

    /// Record one health point per month; keeps the last 13.
    pub fn track_health(&mut self, month: &str, health_percent: f32) {
        if self.health_by_month.iter().any(|(m, _)| m == month) {
            return;
        }
        self.health_by_month
            .push((month.to_string(), health_percent));
        let excess = self.health_by_month.len().saturating_sub(13);
        if excess > 0 {
            self.health_by_month.drain(..excess);
        }
    }

    pub fn health_a_month_ago(&self) -> Option<f32> {
        if self.health_by_month.len() >= 2 {
            self.health_by_month
                .get(self.health_by_month.len() - 2)
                .map(|(_, h)| *h)
        } else {
            None
        }
    }
}

pub fn stats_path() -> Option<PathBuf> {
    Some(crate::config::settings_path()?.with_file_name("stats.json"))
}

pub fn load(path: &Path) -> Stats {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, stats: &Stats) {
    let write = || -> std::io::Result<()> {
        let dir = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(".stats.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(stats)?)?;
        std::fs::rename(&tmp, path)
    };
    if let Err(e) = write() {
        tracing::warn!("failed to save stats: {e}");
    }
}

/// Friday-evening weekly report copy.
pub fn report_message(stats: &Stats, health_now: Option<f32>, lang: &str) -> (String, String) {
    let hours = stats.week_active_secs as f64 / 3600.0;
    let it = lang == "it";
    let title = if it {
        "Il tuo caffè della settimana".to_string()
    } else {
        "Your week in espresso".to_string()
    };
    let health = match (health_now, stats.health_a_month_ago()) {
        (Some(now), Some(ago)) => {
            if it {
                format!(" Salute batteria: {ago:.1}% → {now:.1}%.")
            } else {
                format!(" Battery health: {ago:.1}% → {now:.1}%.")
            }
        }
        _ => String::new(),
    };
    let body = if it {
        format!(
            "{hours:.1} ore di presenza protetta, {} poke, {} espresso completati oggi.{health}",
            stats.week_pokes, stats.shots_today
        )
    } else {
        format!(
            "{hours:.1} hours of protected presence, {} pokes, {} espresso shots today.{health}",
            stats.week_pokes, stats.shots_today
        )
    };
    (title, body)
}

pub fn calibration_message(lang: &str) -> (String, String) {
    if lang == "it" {
        (
            "Calibrazione batteria".to_string(),
            "Una volta al mese: scarica sotto il 10%, poi carica al 100% senza interruzioni. Mantiene precisa la stima dell'autonomia.".to_string(),
        )
    } else {
        (
            "Battery calibration".to_string(),
            "Once a month: drain below 10%, then charge to 100% uninterrupted. Keeps the runtime estimate accurate.".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn week_and_day_rollover() {
        let mut s = Stats::default();
        s.rollover(date("2026-08-04")); // Tuesday
        assert_eq!(s.week_start, "2026-08-03"); // Monday
        s.week_active_secs = 999;
        s.shots_today = 3;

        // Same week, next day: shots reset, week keeps accumulating.
        s.rollover(date("2026-08-05"));
        assert_eq!(s.week_active_secs, 999);
        assert_eq!(s.shots_today, 0);

        // Next week: week bucket resets.
        s.rollover(date("2026-08-10"));
        assert_eq!(s.week_start, "2026-08-10");
        assert_eq!(s.week_active_secs, 0);
    }

    #[test]
    fn poke_tracking_survives_restarts() {
        let mut s = Stats::default();
        s.track_pokes(10);
        assert_eq!(s.week_pokes, 10);
        s.track_pokes(15);
        assert_eq!(s.week_pokes, 15);
        // App restarted: session counter starts over at 3.
        s.track_pokes(3);
        assert_eq!(s.week_pokes, 18);
    }

    #[test]
    fn one_health_point_per_month_max_13() {
        let mut s = Stats::default();
        s.track_health("2026-08", 94.6);
        s.track_health("2026-08", 94.0); // ignored, month already recorded
        assert_eq!(s.health_by_month.len(), 1);
        for i in 0..14 {
            s.track_health(&format!("2027-{:02}", i + 1), 90.0);
        }
        assert!(s.health_by_month.len() <= 13);
        assert_eq!(s.health_a_month_ago(), Some(90.0));
    }
}
