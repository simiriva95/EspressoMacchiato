//! Accessibility trust check. `CGEventPost` on the HID tap silently drops
//! events when the process is not trusted, so this is checked before every
//! poke and re-evaluated on every preflight (grants can change at runtime).

use super::ffi;

pub fn accessibility_trusted() -> bool {
    unsafe { ffi::AXIsProcessTrusted() != 0 }
}

pub const ACCESSIBILITY_HELP: &str =
    "Open System Settings → Privacy & Security → Accessibility and enable EspressoMacchiato.";
