//! Condition probes for the opt-in gates.
//!
//! Power state comes from parsing `pmset -g ps` for now; the M4
//! PowerMonitor will replace it with IOPSCopyPowerSourcesInfo FFI (tracked
//! in docs/IDEAS.md). Screen lock comes from the CGSession dictionary.

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::CFString;

use crate::platform::{ConditionProbe, ProcessMatcher};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// Returns NULL outside a GUI login session. Create rule: we own it.
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

// CoreAudio: is the default input device capturing anywhere? (CoreAudio/
// AudioHardware.h). Selectors are four-char codes.
#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE: u32 = u32::from_be_bytes(*b"dIn ");
const K_AUDIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE: u32 = u32::from_be_bytes(*b"gone");
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyData(
        object_id: u32,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const std::ffi::c_void,
        data_size: *mut u32,
        data: *mut std::ffi::c_void,
    ) -> i32;
}

/// True while ANY process captures from the default input device — exactly
/// the "am I in a call" signal. Read-only: no mic permission needed, we
/// never touch audio data.
fn default_input_running() -> Option<bool> {
    unsafe {
        let addr = AudioObjectPropertyAddress {
            selector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE,
            scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };
        let mut device_id: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        if AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut device_id as *mut u32 as *mut _,
        ) != 0
            || device_id == 0
        {
            return None;
        }

        let addr = AudioObjectPropertyAddress {
            selector: K_AUDIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE,
            scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };
        let mut running: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        if AudioObjectGetPropertyData(
            device_id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut running as *mut u32 as *mut _,
        ) != 0
        {
            return None;
        }
        Some(running != 0)
    }
}

fn session_screen_locked() -> Option<bool> {
    let raw = unsafe { CGSessionCopyCurrentDictionary() };
    if raw.is_null() {
        return None;
    }
    // Safety: non-null, owned per the Copy rule.
    let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_create_rule(raw) };
    let key = CFString::from_static_string("CGSSessionScreenIsLocked");
    match dict.find(&key) {
        // Key present only while locked, but read the value anyway.
        Some(value) => Some(
            value
                .downcast::<CFBoolean>()
                .map(bool::from)
                .unwrap_or(true),
        ),
        None => Some(false),
    }
}

// ponytail: shelling to pmset until the M4 IOKit PowerMonitor lands.
fn pmset_ps() -> Option<String> {
    let out = std::process::Command::new("pmset")
        .args(["-g", "ps"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse_percent(ps: &str) -> Option<f32> {
    // Line looks like: " -InternalBattery-0 (id=…)\t85%; discharging; …"
    let idx = ps.find('%')?;
    let digits: String = ps[..idx]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

pub struct MacConditionProbe {
    processes: ProcessMatcher,
}

impl MacConditionProbe {
    pub fn new() -> Self {
        Self {
            processes: ProcessMatcher::new(),
        }
    }
}

impl Default for MacConditionProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl ConditionProbe for MacConditionProbe {
    fn on_ac(&self) -> Option<bool> {
        pmset_ps().map(|ps| ps.contains("AC Power"))
    }

    fn battery_percent(&self) -> Option<f32> {
        parse_percent(&pmset_ps()?)
    }

    fn screen_locked(&self) -> Option<bool> {
        session_screen_locked()
    }

    fn any_process_running(&self, names: &[String]) -> bool {
        self.processes.any_running(names)
    }

    fn mic_in_use(&self) -> Option<bool> {
        default_input_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_battery_percent_from_pmset_output() {
        let ps = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=12345)\t85%; discharging; 4:20 remaining present: true\n";
        assert_eq!(parse_percent(ps), Some(85.0));
        assert_eq!(parse_percent("no percent here"), None);
    }
}
