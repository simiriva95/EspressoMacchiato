//! Schedule windows evaluated on the local wall clock.
//!
//! Deliberately polling-based (the engine re-evaluates every gate tick on
//! `Local::now()`), never precomputed absolute deadlines: a DST jump or a
//! suspend/resume cannot make a window fire late or be skipped — the next
//! evaluation simply sees the new wall time. A window like 09:00–18:00
//! means wall-clock 09–18 regardless of what UTC did that night.

use chrono::{Datelike, NaiveDateTime, NaiveTime};

use crate::config::ScheduleWindow;

/// "HH:MM" → NaiveTime. Invalid strings disable the window (logged once by
/// the caller, not a crash).
pub fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M").ok()
}

/// Day of week as 0 = Monday … 6 = Sunday, matching the config encoding.
fn weekday_index(dt: &NaiveDateTime) -> u8 {
    dt.weekday().num_days_from_monday() as u8
}

fn previous_day(day: u8) -> u8 {
    (day + 6) % 7
}

/// Is any window open at `now`? Start inclusive, end exclusive.
/// A window with `end <= start` crosses midnight: it covers
/// [start, 24:00) on each listed day and [00:00, end) on the following day.
pub fn is_open(windows: &[ScheduleWindow], now: NaiveDateTime) -> bool {
    let today = weekday_index(&now);
    let time = now.time();
    windows.iter().any(|w| {
        let (Some(start), Some(end)) = (parse_hhmm(&w.start), parse_hhmm(&w.end)) else {
            return false;
        };
        if start < end {
            w.days.contains(&today) && time >= start && time < end
        } else {
            // Crosses midnight (or zero-length treated as crossing to 00:00).
            (w.days.contains(&today) && time >= start)
                || (w.days.contains(&previous_day(today)) && time < end)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn win(days: &[u8], start: &str, end: &str) -> ScheduleWindow {
        ScheduleWindow {
            days: days.to_vec(),
            start: start.into(),
            end: end.into(),
        }
    }

    /// year/month/day hh:mm as NaiveDateTime.
    fn at(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(hh, mm, 0)
            .unwrap()
    }

    #[test]
    fn weekday_window_boundaries() {
        // Mon–Fri 09:00–18:00. 2026-08-04 is a Tuesday.
        let w = [win(&[0, 1, 2, 3, 4], "09:00", "18:00")];
        assert!(is_open(&w, at(2026, 8, 4, 9, 0)), "start inclusive");
        assert!(is_open(&w, at(2026, 8, 4, 12, 30)));
        assert!(!is_open(&w, at(2026, 8, 4, 18, 0)), "end exclusive");
        assert!(!is_open(&w, at(2026, 8, 4, 8, 59)));
        assert!(!is_open(&w, at(2026, 8, 8, 12, 0)), "Saturday closed");
    }

    #[test]
    fn midnight_crossing_window() {
        // Friday 22:00 → 02:00. 2026-08-07 is a Friday.
        let w = [win(&[4], "22:00", "02:00")];
        assert!(is_open(&w, at(2026, 8, 7, 23, 0)), "Friday night");
        assert!(is_open(&w, at(2026, 8, 8, 1, 59)), "spills into Saturday");
        assert!(!is_open(&w, at(2026, 8, 8, 2, 0)), "end exclusive");
        assert!(!is_open(&w, at(2026, 8, 7, 21, 59)));
        assert!(
            !is_open(&w, at(2026, 8, 9, 1, 0)),
            "Sunday morning is not covered (only Fri→Sat)"
        );
    }

    #[test]
    fn multiple_windows_same_day() {
        let w = [
            win(&[0, 1, 2, 3, 4], "09:00", "12:30"),
            win(&[0, 1, 2, 3, 4], "14:00", "18:00"),
        ];
        assert!(is_open(&w, at(2026, 8, 4, 10, 0)));
        assert!(!is_open(&w, at(2026, 8, 4, 13, 0)), "lunch gap");
        assert!(is_open(&w, at(2026, 8, 4, 15, 0)));
    }

    #[test]
    fn dst_transition_days_evaluate_on_wall_clock() {
        // Evaluation is wall-clock only, so DST cannot skip a window: on the
        // EU spring-forward night (2026-03-29, a Sunday, 02:00→03:00) the
        // 00:00–04:00 window is still open at wall 03:30, and on fall-back
        // (2026-10-25) 02:30 — which occurs twice in UTC — is simply "02:30".
        let w = [win(&[6], "00:00", "04:00")];
        assert!(is_open(&w, at(2026, 3, 29, 3, 30)));
        assert!(is_open(&w, at(2026, 10, 25, 2, 30)));
        assert!(!is_open(&w, at(2026, 3, 29, 4, 30)));
    }

    #[test]
    fn invalid_times_disable_the_window_without_panicking() {
        let w = [win(&[1], "9am", "18:00")];
        assert!(!is_open(&w, at(2026, 8, 4, 10, 0)));
    }
}
