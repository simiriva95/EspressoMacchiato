//! System idle time as macOS sees it — and as Electron clients (Teams,
//! Slack) read it via powerMonitor.getSystemIdleTime().

use super::ffi;
use crate::platform::{IdleReader, PlatformError};

pub struct MacIdleReader;

impl IdleReader for MacIdleReader {
    fn idle_seconds(&self) -> Result<f64, PlatformError> {
        // Safety: pure query, no ownership involved.
        let secs = unsafe {
            ffi::CGEventSourceSecondsSinceLastEventType(
                ffi::COMBINED_SESSION_STATE,
                ffi::ANY_INPUT_EVENT_TYPE,
            )
        };
        Ok(secs)
    }

    fn source(&self) -> &'static str {
        "CGEventSource"
    }
}
