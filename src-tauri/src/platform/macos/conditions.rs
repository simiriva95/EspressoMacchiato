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
