pub mod activity;
pub mod ffi;
pub mod idle;
pub mod inhibitor;
pub mod permissions;

use std::sync::Arc;

use super::{Degradation, DegradationKind, NullPowerMonitor, Platform};

pub fn platform() -> Platform {
    Platform {
        inhibitor: Box::new(inhibitor::MacInhibitor::default()),
        simulator: Box::new(activity::MacActivitySimulator::new()),
        idle: Box::new(idle::MacIdleReader),
        power: Box::new(NullPowerMonitor),
        preflight: Arc::new(|| {
            if permissions::accessibility_trusted() {
                Vec::new()
            } else {
                vec![Degradation {
                    what: DegradationKind::PokeUnavailable,
                    detail: "Accessibility permission not granted: cannot inject the synthetic activity that resets the idle counter.".into(),
                    help: Some(permissions::ACCESSIBILITY_HELP.into()),
                }]
            }
        }),
    }
}

#[cfg(test)]
mod hardware_tests {
    //! Real-hardware tests, `#[ignore]` by default. Run locally with:
    //! `cargo test -- --ignored --nocapture` (see CONTRIBUTING).

    use super::*;
    use crate::platform::{ActivitySimulator, IdleReader, InhibitOptions, SleepInhibitor};

    #[test]
    #[ignore = "needs real macOS session"]
    fn idle_reader_returns_plausible_value() {
        let reader = idle::MacIdleReader;
        let secs = reader.idle_seconds().expect("idle readable");
        println!("idle_seconds = {secs}");
        assert!((0.0..86_400.0).contains(&secs));
    }

    #[test]
    #[ignore = "needs real macOS session; verify with `pmset -g assertions`"]
    fn inhibitor_creates_and_releases_assertions() {
        let mut inhibitor = inhibitor::MacInhibitor::default();
        inhibitor
            .acquire(InhibitOptions::default())
            .expect("acquire");
        assert!(inhibitor.is_active());
        // Long enough to eyeball `pmset -g assertions` in another shell if
        // run with --nocapture and a breakpoint; kept short for automation.
        let out = std::process::Command::new("pmset")
            .args(["-g", "assertions"])
            .output()
            .expect("pmset runs");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        println!("{text}");
        assert!(text.contains("PreventUserIdleSystemSleep"));
        assert!(text.contains("PreventUserIdleDisplaySleep"));
        inhibitor.release().expect("release");
        assert!(!inhibitor.is_active());
    }

    #[test]
    #[ignore = "needs real macOS session + Accessibility trust"]
    fn zero_mouse_move_resets_idle_counter() {
        let reader = idle::MacIdleReader;
        let mut sim = activity::MacActivitySimulator::new();
        // Wait so idle accumulates measurably above the post-poke value.
        std::thread::sleep(std::time::Duration::from_millis(2500));
        let before = reader.idle_seconds().unwrap();
        sim.poke().expect("poke (needs Accessibility)");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let after = reader.idle_seconds().unwrap();
        println!("idle before = {before:.2}s, after = {after:.2}s");
        assert!(before >= 2.0, "test precondition: idle accumulated");
        assert!(after < 1.0, "poke must reset the OS idle counter");
    }
}
