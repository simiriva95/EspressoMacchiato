//! Synthetic activity via CoreGraphics event injection on the HID tap.

use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

use super::permissions;
use crate::platform::{ActivitySimulator, ActivityStrategy, PlatformError};

/// kVK_F15 (Carbon HIToolbox Events.h).
const KEYCODE_F15: u16 = 0x71;

pub struct MacActivitySimulator {
    strategy: ActivityStrategy,
}

impl MacActivitySimulator {
    pub fn new() -> Self {
        Self {
            strategy: ActivityStrategy::ZeroMouseMove,
        }
    }
}

fn source() -> Result<CGEventSource, PlatformError> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| PlatformError::Other("CGEventSourceCreate failed".into()))
}

fn cursor_position(src: &CGEventSource) -> Result<CGPoint, PlatformError> {
    // CGEventCreate(NULL-ish source) yields an event whose location is the
    // current cursor position, already in CGEvent (top-left origin) space —
    // no NSEvent coordinate flip needed.
    CGEvent::new(src.clone())
        .map(|e| e.location())
        .map_err(|_| PlatformError::Other("CGEventCreate failed".into()))
}

fn post_mouse_move(src: &CGEventSource, pos: CGPoint) -> Result<(), PlatformError> {
    let event = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::MouseMoved,
        pos,
        CGMouseButton::Left,
    )
    .map_err(|_| PlatformError::Other("CGEventCreateMouseEvent failed".into()))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}

impl ActivitySimulator for MacActivitySimulator {
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
        // CGEventPost silently drops events without Accessibility trust —
        // check explicitly so the failure is visible instead of silent.
        if !permissions::accessibility_trusted() {
            return Err(PlatformError::PermissionDenied(
                "Accessibility permission not granted".into(),
            ));
        }
        let src = source()?;
        match self.strategy {
            ActivityStrategy::ZeroMouseMove => {
                let pos = cursor_position(&src)?;
                post_mouse_move(&src, pos)
            }
            ActivityStrategy::HarmlessKeyTap => {
                for down in [true, false] {
                    let event = CGEvent::new_keyboard_event(src.clone(), KEYCODE_F15, down)
                        .map_err(|_| {
                            PlatformError::Other("CGEventCreateKeyboardEvent failed".into())
                        })?;
                    event.post(CGEventTapLocation::HID);
                }
                Ok(())
            }
            ActivityStrategy::NudgeAndReturn => {
                let pos = cursor_position(&src)?;
                let nudged = CGPoint::new(pos.x + 1.0, pos.y);
                post_mouse_move(&src, nudged)?;
                post_mouse_move(&src, pos)
            }
        }
    }
}
