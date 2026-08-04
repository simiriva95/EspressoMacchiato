//! Default activity backend: a virtual pointer on /dev/uinput. Works on
//! X11 *and* Wayland because the event enters at kernel level.
//!
//! The device declares REL_X/REL_Y plus BTN_LEFT (some compositors ignore
//! pointer devices with no buttons) and KEY_F15 for the key-tap strategy.
//! /dev/uinput is root-only by default: see
//! packaging/linux/99-espressomacchiato-uinput.rules.

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};

use crate::platform::{ActivitySimulator, ActivityStrategy, PlatformError};

pub const UINPUT_HELP: &str = "Install the udev rule and join the input group:\n  sudo cp 99-espressomacchiato-uinput.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules && sudo udevadm trigger\n  sudo usermod -aG input $USER\nthen log out and back in.";

pub struct UinputSimulator {
    device: VirtualDevice,
    strategy: ActivityStrategy,
}

impl UinputSimulator {
    pub fn new() -> Result<Self, PlatformError> {
        let mut axes = AttributeSet::<RelativeAxisCode>::new();
        axes.insert(RelativeAxisCode::REL_X);
        axes.insert(RelativeAxisCode::REL_Y);
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::KEY_F15);

        let device = VirtualDevice::builder()
            .map_err(map_uinput_err)?
            .name("EspressoMacchiato virtual pointer")
            .with_relative_axes(&axes)
            .map_err(map_uinput_err)?
            .with_keys(&keys)
            .map_err(map_uinput_err)?
            .build()
            .map_err(map_uinput_err)?;

        Ok(Self {
            device,
            strategy: ActivityStrategy::ZeroMouseMove,
        })
    }

    fn emit_rel(&mut self, dx: i32, dy: i32) -> Result<(), PlatformError> {
        // emit() appends the SYN_REPORT itself.
        self.device
            .emit(&[
                InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, dx),
                InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, dy),
            ])
            .map_err(map_uinput_err)
    }

    fn emit_key_tap(&mut self) -> Result<(), PlatformError> {
        for value in [1, 0] {
            self.device
                .emit(&[InputEvent::new(EventType::KEY.0, KeyCode::KEY_F15.0, value)])
                .map_err(map_uinput_err)?;
        }
        Ok(())
    }
}

fn map_uinput_err(e: std::io::Error) -> PlatformError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        PlatformError::PermissionDenied(format!("/dev/uinput not writable: {e}"))
    } else {
        PlatformError::Io(e)
    }
}

impl ActivitySimulator for UinputSimulator {
    fn strategy(&self) -> ActivityStrategy {
        self.strategy
    }

    fn available_strategies(&self) -> Vec<ActivityStrategy> {
        vec![
            ActivityStrategy::ZeroMouseMove,
            ActivityStrategy::HarmlessKeyTap,
            ActivityStrategy::NudgeAndReturn,
        ]
    }

    fn set_strategy(&mut self, s: ActivityStrategy) -> Result<(), PlatformError> {
        self.strategy = s;
        Ok(())
    }

    fn poke(&mut self) -> Result<(), PlatformError> {
        match self.strategy {
            ActivityStrategy::ZeroMouseMove => self.emit_rel(0, 0),
            ActivityStrategy::HarmlessKeyTap => self.emit_key_tap(),
            ActivityStrategy::NudgeAndReturn => {
                self.emit_rel(1, 0)?;
                self.emit_rel(-1, 0)
            }
        }
    }
}
