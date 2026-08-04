//! Battery and system metrics from /sys/class/power_supply. Some kernels
//! expose charge_* (µAh), others energy_* (µWh): both are handled and
//! normalized. Paths are parameterized so tests run against a fake sysfs.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::platform::{PlatformError, PowerMonitor, PowerSnapshot, SystemMetrics};

pub struct LinuxPowerMonitor {
    base: PathBuf,
    system: SystemMetrics,
}

impl LinuxPowerMonitor {
    pub fn new() -> Self {
        Self::with_base("/sys/class/power_supply")
    }

    pub fn with_base(base: impl Into<PathBuf>) -> Self {
        Self {
            base: base.into(),
            system: SystemMetrics::new(),
        }
    }
}

impl Default for LinuxPowerMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn read<T: std::str::FromStr>(dir: &Path, name: &str) -> Option<T> {
    std::fs::read_to_string(dir.join(name))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn read_string(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Battery charge/energy state, normalized. charge_* is µAh (→ mAh),
/// energy_* is µWh (health ratio only: mAh needs a voltage assumption).
fn fill_battery(snapshot: &mut PowerSnapshot, dir: &Path) {
    snapshot.percent = read::<f32>(dir, "capacity");
    snapshot.cycle_count = read::<u32>(dir, "cycle_count");
    snapshot.voltage_v = read::<f64>(dir, "voltage_now").map(|uv| (uv / 1e6) as f32);
    // 0.1 °C units.
    snapshot.temperature_c = read::<f64>(dir, "temp").map(|t| (t / 10.0) as f32);

    let current_ua = read::<f64>(dir, "current_now");
    snapshot.amperage_ma = current_ua.map(|ua| (ua / 1000.0) as i32);

    // Watts: power_now when present, else V * A.
    snapshot.watts = read::<f64>(dir, "power_now")
        .map(|uw| (uw / 1e6) as f32)
        .or(match (snapshot.voltage_v, current_ua) {
            (Some(v), Some(ua)) => Some((v as f64 * ua / 1e6).abs() as f32),
            _ => None,
        });

    // Capacity + health, from whichever family the kernel exposes.
    let charge_full = read::<f64>(dir, "charge_full");
    let charge_design = read::<f64>(dir, "charge_full_design");
    let energy_full = read::<f64>(dir, "energy_full");
    let energy_design = read::<f64>(dir, "energy_full_design");
    snapshot.max_capacity_mah = charge_full.map(|uah| (uah / 1000.0) as u32);
    snapshot.design_capacity_mah = charge_design.map(|uah| (uah / 1000.0) as u32);
    snapshot.health_percent = match (charge_full, charge_design) {
        (Some(full), Some(design)) if design > 0.0 => Some((full / design * 100.0) as f32),
        _ => match (energy_full, energy_design) {
            (Some(full), Some(design)) if design > 0.0 => Some((full / design * 100.0) as f32),
            _ => None,
        },
    };

    // Remaining time, best effort from instantaneous draw.
    let status = read_string(dir, "status").unwrap_or_default();
    let stored = read::<f64>(dir, "energy_now")
        .zip(read::<f64>(dir, "power_now"))
        .or(read::<f64>(dir, "charge_now")
            .zip(current_ua)
            .map(|(c, i)| (c, i.abs())));
    if let Some((amount, rate)) = stored {
        if rate > 0.0 {
            let hours = amount / rate;
            let duration = Duration::from_secs((hours * 3600.0) as u64);
            match status.as_str() {
                "Discharging" => snapshot.time_to_empty = Some(duration),
                "Charging" => {
                    // Rough: time to fill the missing part at current rate.
                    let full = charge_full.or(energy_full);
                    if let Some(full) = full {
                        let missing = (full - amount).max(0.0);
                        snapshot.time_to_full =
                            Some(Duration::from_secs((missing / rate * 3600.0) as u64));
                    }
                }
                _ => {}
            }
        }
    }
}

/// Scan the sysfs base dir; returns (snapshot, found_any_supply).
fn scan(base: &Path) -> (PowerSnapshot, bool) {
    let mut snapshot = PowerSnapshot::default();
    let mut found = false;
    let Ok(entries) = std::fs::read_dir(base) else {
        return (snapshot, false);
    };
    let mut ac_online: Option<bool> = None;
    for entry in entries.flatten() {
        let dir = entry.path();
        match read_string(&dir, "type").as_deref() {
            Some("Mains") => {
                found = true;
                let online = read::<u8>(&dir, "online") == Some(1);
                ac_online = Some(ac_online.unwrap_or(false) || online);
            }
            Some("Battery") if snapshot.percent.is_none() => {
                found = true;
                fill_battery(&mut snapshot, &dir);
            }
            _ => {}
        }
    }
    // No Mains supply but a battery that isn't discharging → on AC.
    snapshot.on_ac = ac_online.unwrap_or(false);
    (snapshot, found)
}

impl PowerMonitor for LinuxPowerMonitor {
    fn snapshot(&self) -> Result<PowerSnapshot, PlatformError> {
        let (mut snapshot, _found) = scan(&self.base);
        self.system.fill(&mut snapshot);
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_sysfs(kind: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "espresso-sysfs-{kind}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(base.join("BAT0")).unwrap();
        std::fs::create_dir_all(base.join("AC")).unwrap();
        std::fs::write(base.join("AC/type"), "Mains\n").unwrap();
        std::fs::write(base.join("BAT0/type"), "Battery\n").unwrap();
        base
    }

    fn write(base: &Path, rel: &str, value: &str) {
        std::fs::write(base.join(rel), format!("{value}\n")).unwrap();
    }

    #[test]
    fn charge_family_is_normalized_to_mah() {
        let base = fake_sysfs("charge");
        write(&base, "AC/online", "0");
        write(&base, "BAT0/status", "Discharging");
        write(&base, "BAT0/capacity", "73");
        write(&base, "BAT0/cycle_count", "142");
        write(&base, "BAT0/voltage_now", "11500000"); // 11.5 V
        write(&base, "BAT0/current_now", "1200000"); // 1.2 A
        write(&base, "BAT0/charge_now", "3000000"); // 3000 mAh
        write(&base, "BAT0/charge_full", "4100000");
        write(&base, "BAT0/charge_full_design", "5000000");

        let (s, found) = scan(&base);
        assert!(found);
        assert!(!s.on_ac);
        assert_eq!(s.percent, Some(73.0));
        assert_eq!(s.cycle_count, Some(142));
        assert_eq!(s.max_capacity_mah, Some(4100));
        assert_eq!(s.design_capacity_mah, Some(5000));
        assert_eq!(s.health_percent, Some(82.0));
        assert_eq!(s.voltage_v, Some(11.5));
        assert_eq!(s.amperage_ma, Some(1200));
        // 11.5 V * 1.2 A = 13.8 W
        assert!((s.watts.unwrap() - 13.8).abs() < 0.01);
        // 3000 mAh / 1200 mA = 2.5 h
        assert_eq!(s.time_to_empty, Some(Duration::from_secs(9000)));
    }

    #[test]
    fn energy_family_gives_health_and_watts() {
        let base = fake_sysfs("energy");
        write(&base, "AC/online", "1");
        write(&base, "BAT0/status", "Charging");
        write(&base, "BAT0/capacity", "55");
        write(&base, "BAT0/power_now", "30000000"); // 30 W
        write(&base, "BAT0/energy_now", "30000000"); // 30 Wh
        write(&base, "BAT0/energy_full", "60000000");
        write(&base, "BAT0/energy_full_design", "75000000");

        let (s, _) = scan(&base);
        assert!(s.on_ac);
        assert_eq!(s.percent, Some(55.0));
        assert_eq!(s.watts, Some(30.0));
        assert_eq!(s.health_percent, Some(80.0));
        // mAh not derivable from µWh: must stay None, not a fake zero.
        assert_eq!(s.max_capacity_mah, None);
        // (60 - 30) Wh at 30 W → 1 h to full.
        assert_eq!(s.time_to_full, Some(Duration::from_secs(3600)));
        assert_eq!(s.time_to_empty, None);
    }

    #[test]
    fn desktop_without_battery_reports_nothing_fake() {
        let base =
            std::env::temp_dir().join(format!("espresso-sysfs-empty-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let (s, found) = scan(&base);
        assert!(!found);
        assert_eq!(s.percent, None);
        assert_eq!(s.watts, None);
        assert_eq!(s.health_percent, None);
    }
}
