//! "Until end of meeting": EventKit lookup of the event you are in right
//! now (or one starting within the hour). Read-only, opt-in — the system
//! Calendar permission dialog appears on first use, driven by the user
//! picking the option, never in the background.

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore};
use objc2_foundation::{NSDate, NSError};

const LOOKAHEAD_SECS: f64 = 8.0 * 3600.0;
const SOON_SECS: f64 = 3600.0;

/// Unix seconds when the current (or imminent) meeting ends. Ok(None) =
/// permission granted but no relevant event. Err = no permission.
pub fn next_meeting_end() -> Result<Option<i64>, String> {
    // Safety: all EventKit calls follow the documented API; the store and
    // every returned object are retained by objc2.
    unsafe {
        let store = EKEventStore::new();

        let status = EKEventStore::authorizationStatusForEntityType(EKEntityType::Event);
        let authorized = if status == EKAuthorizationStatus::FullAccess {
            true
        } else if status == EKAuthorizationStatus::NotDetermined {
            // Blocks until the user answers the system dialog (or 2 min).
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            let completion = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
                let _ = tx.send(granted.as_bool());
            });
            store.requestFullAccessToEventsWithCompletion(&*completion as *const _ as *mut _);
            rx.recv_timeout(std::time::Duration::from_secs(120))
                .unwrap_or(false)
        } else {
            false
        };
        if !authorized {
            return Err("calendar access not granted".into());
        }

        let start = NSDate::now();
        let end = NSDate::dateWithTimeIntervalSinceNow(LOOKAHEAD_SECS);
        let predicate = store.predicateForEventsWithStartDate_endDate_calendars(&start, &end, None);
        let events = store.eventsMatchingPredicate(&predicate);

        let now = start.timeIntervalSince1970();
        let mut ongoing_end: Option<f64> = None; // max end among events covering now
        let mut upcoming: Option<(f64, f64)> = None; // (start, end) of the next one

        for event in events.iter() {
            if event.isAllDay() {
                continue;
            }
            let ev_start = event.startDate().timeIntervalSince1970();
            let ev_end = event.endDate().timeIntervalSince1970();
            if ev_start <= now && ev_end > now {
                ongoing_end = Some(ongoing_end.map_or(ev_end, |current| current.max(ev_end)));
            } else if ev_start > now && ev_start - now <= SOON_SECS {
                match upcoming {
                    Some((s, _)) if s <= ev_start => {}
                    _ => upcoming = Some((ev_start, ev_end)),
                }
            }
        }

        Ok(ongoing_end
            .or(upcoming.map(|(_, e)| e))
            .map(|end| end as i64))
    }
}
