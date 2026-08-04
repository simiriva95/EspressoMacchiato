//! Battery and system metrics via IOKit's AppleSmartBattery registry entry.
//! Every field stays Option: hardware and macOS versions differ, and the UI
//! must say "not available", never show a fake zero.

use std::time::Duration;

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

use super::ffi;
use crate::platform::{PlatformError, PowerMonitor, PowerSnapshot, SystemMetrics};

/// AppleSmartBattery's TimeRemaining sentinel for "unknown".
const TIME_REMAINING_UNKNOWN: i64 = 65_535;

pub struct MacPowerMonitor {
    system: SystemMetrics,
}

impl MacPowerMonitor {
    pub fn new() -> Self {
        Self {
            system: SystemMetrics::new(),
        }
    }
}

impl Default for MacPowerMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn battery_properties() -> Result<CFDictionary<CFString, CFType>, PlatformError> {
    // Safety: matching dict is consumed by IOServiceGetMatchingService; the
    // returned service and properties are released by us.
    unsafe {
        let matching = ffi::IOServiceMatching(c"AppleSmartBattery".as_ptr());
        if matching.is_null() {
            return Err(PlatformError::Other("IOServiceMatching failed".into()));
        }
        let service = ffi::IOServiceGetMatchingService(0, matching);
        if service == 0 {
            return Err(PlatformError::Unsupported(
                "no AppleSmartBattery service (desktop Mac?)".into(),
            ));
        }
        let mut props: core_foundation::dictionary::CFMutableDictionaryRef = std::ptr::null_mut();
        let ret = ffi::IORegistryEntryCreateCFProperties(service, &mut props, std::ptr::null(), 0);
        ffi::IOObjectRelease(service);
        if ret != ffi::kIOReturnSuccess || props.is_null() {
            return Err(PlatformError::Other(format!(
                "IORegistryEntryCreateCFProperties failed: {ret:#x}"
            )));
        }
        Ok(CFDictionary::wrap_under_create_rule(props as _))
    }
}

fn get_i64(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<i64> {
    dict.find(CFString::new(key))?
        .downcast::<CFNumber>()?
        .to_i64()
}

fn get_bool(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<bool> {
    Some(
        dict.find(CFString::new(key))?
            .downcast::<CFBoolean>()?
            .into(),
    )
}

impl PowerMonitor for MacPowerMonitor {
    fn snapshot(&self) -> Result<PowerSnapshot, PlatformError> {
        let mut snapshot = PowerSnapshot::default();

        match battery_properties() {
            Ok(dict) => {
                snapshot.on_ac = get_bool(&dict, "ExternalConnected").unwrap_or(false);
                snapshot.cycle_count = get_i64(&dict, "CycleCount").map(|v| v as u32);
                snapshot.design_capacity_mah = get_i64(&dict, "DesignCapacity").map(|v| v as u32);
                snapshot.max_capacity_mah = get_i64(&dict, "AppleRawMaxCapacity").map(|v| v as u32);
                snapshot.health_percent =
                    match (snapshot.max_capacity_mah, snapshot.design_capacity_mah) {
                        (Some(max), Some(design)) if design > 0 => {
                            Some(max as f32 / design as f32 * 100.0)
                        }
                        _ => None,
                    };
                // Hundredths of °C.
                snapshot.temperature_c = get_i64(&dict, "Temperature").map(|v| v as f32 / 100.0);
                // mV.
                snapshot.voltage_v = get_i64(&dict, "Voltage").map(|v| v as f32 / 1000.0);
                // mA, negative while discharging. Some models store it as an
                // unsigned 64-bit two's complement: truncating to i32
                // recovers the sign.
                snapshot.amperage_ma = get_i64(&dict, "Amperage").map(|v| v as i32);
                snapshot.watts = match (snapshot.voltage_v, snapshot.amperage_ma) {
                    (Some(v), Some(ma)) => Some((v * ma as f32 / 1000.0).abs()),
                    _ => None,
                };

                // Percent: Apple Silicon reports CurrentCapacity directly in
                // percent; Intel reports raw mAh, so derive from raw values.
                snapshot.percent = match get_i64(&dict, "CurrentCapacity") {
                    Some(v) if (0..=100).contains(&v) => Some(v as f32),
                    _ => match (
                        get_i64(&dict, "AppleRawCurrentCapacity"),
                        get_i64(&dict, "AppleRawMaxCapacity"),
                    ) {
                        (Some(current), Some(max)) if max > 0 => {
                            Some(current as f32 / max as f32 * 100.0)
                        }
                        _ => None,
                    },
                };

                let charging = get_bool(&dict, "IsCharging").unwrap_or(false);
                let remaining = get_i64(&dict, "TimeRemaining")
                    .filter(|v| (1..TIME_REMAINING_UNKNOWN).contains(v))
                    .map(|minutes| Duration::from_secs(minutes as u64 * 60));
                if charging {
                    snapshot.time_to_full = remaining;
                } else if !snapshot.on_ac {
                    snapshot.time_to_empty = remaining;
                }
            }
            Err(PlatformError::Unsupported(_)) => {
                // Desktop Mac: no battery, AC by definition. Battery fields
                // stay None and the UI labels them not available.
                snapshot.on_ac = true;
            }
            Err(e) => return Err(e),
        }

        self.system.fill(&mut snapshot);
        Ok(snapshot)
    }
}

#[cfg(test)]
mod hardware_tests {
    use super::*;

    #[test]
    #[ignore = "needs real macOS hardware"]
    fn reads_a_plausible_battery_snapshot() {
        let monitor = MacPowerMonitor::new();
        let s = monitor.snapshot().expect("snapshot");
        println!("{s:#?}");
        if let Some(p) = s.percent {
            assert!((0.0..=100.0).contains(&p));
        }
        if let Some(t) = s.temperature_c {
            assert!((0.0..=80.0).contains(&t), "temp {t} plausible");
        }
        assert!(s.memory_total_bytes > 0);
    }
}
