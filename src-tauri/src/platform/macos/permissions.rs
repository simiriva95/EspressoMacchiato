//! Accessibility trust check. `CGEventPost` on the HID tap silently drops
//! events when the process is not trusted, so this is checked before every
//! poke and re-evaluated on every preflight (grants can change at runtime).

use super::ffi;

pub fn accessibility_trusted() -> bool {
    unsafe { ffi::AXIsProcessTrusted() != 0 }
}

/// Show the system Accessibility prompt (and register the app in the
/// Privacy & Security list). Returns the current trust state; macOS only
/// shows the dialog the first time, later calls are silent.
pub fn request_accessibility() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    // Safety: kAXTrustedCheckOptionPrompt is a static owned by the
    // framework; wrap_under_get_rule retains it.
    unsafe {
        let key = CFString::wrap_under_get_rule(ffi::kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        ffi::AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
    }
}

pub const ACCESSIBILITY_HELP: &str =
    "Open System Settings → Privacy & Security → Accessibility and enable EspressoMacchiato.";
