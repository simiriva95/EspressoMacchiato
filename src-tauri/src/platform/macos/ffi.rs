//! Hand-declared externs for APIs no maintained crate binds.
//! Signatures from Apple SDK headers (IOPMLib.h, CGEventSource.h,
//! AXUIElement.h). See docs/PLATFORM-NOTES.md.

#![allow(non_upper_case_globals)]

use core_foundation::string::CFStringRef;

pub type IOPMAssertionID = u32;
pub type IOReturn = i32;

pub const kIOPMAssertionLevelOn: u32 = 255;
pub const kIOReturnSuccess: IOReturn = 0;

// Assertion type strings from IOPMLib.h.
pub const ASSERTION_PREVENT_SYSTEM_SLEEP: &str = "PreventUserIdleSystemSleep";
pub const ASSERTION_PREVENT_DISPLAY_SLEEP: &str = "PreventUserIdleDisplaySleep";

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    pub fn IOPMAssertionCreateWithName(
        assertion_type: CFStringRef,
        assertion_level: u32,
        assertion_name: CFStringRef,
        assertion_id: *mut IOPMAssertionID,
    ) -> IOReturn;
    pub fn IOPMAssertionRelease(assertion_id: IOPMAssertionID) -> IOReturn;
}

/// kCGEventSourceStateCombinedSessionState (CGEventSource.h) — the state
/// Electron's powerMonitor reads, hence the metric Teams-like clients see.
pub const COMBINED_SESSION_STATE: i32 = 0;
/// kCGAnyInputEventType (CGEventTypes.h): `(CGEventType)(~0)`.
pub const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// Returns a C `Boolean` (unsigned char).
    pub fn AXIsProcessTrusted() -> u8;
}
