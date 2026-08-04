//! Fallback activity backend: XTest fake relative motion. Zero special
//! permissions, X11 only. Under XWayland it resets the X server's idle,
//! not the compositor's — documented, not promised.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::MOTION_NOTIFY_EVENT;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::platform::{ActivitySimulator, ActivityStrategy, PlatformError};

pub struct XTestSimulator {
    conn: RustConnection,
    strategy: ActivityStrategy,
}

impl XTestSimulator {
    pub fn new() -> Result<Self, PlatformError> {
        let (conn, _screen) = x11rb::connect(None)
            .map_err(|e| PlatformError::Unsupported(format!("no X server: {e}")))?;
        Ok(Self {
            conn,
            strategy: ActivityStrategy::ZeroMouseMove,
        })
    }

    /// XTestFakeInput with MotionNotify and detail=1 → relative motion.
    fn fake_relative_motion(&self, dx: i16, dy: i16) -> Result<(), PlatformError> {
        self.conn
            .xtest_fake_input(MOTION_NOTIFY_EVENT, 1, 0, x11rb::NONE, dx, dy, 0)
            .map_err(|e| PlatformError::Other(format!("XTest FakeInput: {e}")))?;
        self.conn
            .flush()
            .map_err(|e| PlatformError::Other(format!("X flush: {e}")))?;
        Ok(())
    }
}

impl ActivitySimulator for XTestSimulator {
    fn strategy(&self) -> ActivityStrategy {
        self.strategy
    }

    fn available_strategies(&self) -> Vec<ActivityStrategy> {
        // Key taps via XTest need keysym→keycode mapping; not worth it while
        // uinput is the primary backend. Mouse strategies only.
        vec![
            ActivityStrategy::ZeroMouseMove,
            ActivityStrategy::NudgeAndReturn,
        ]
    }

    fn set_strategy(&mut self, s: ActivityStrategy) -> Result<(), PlatformError> {
        if !self.available_strategies().contains(&s) {
            return Err(PlatformError::Unsupported(
                "strategy not available on the XTest backend".into(),
            ));
        }
        self.strategy = s;
        Ok(())
    }

    fn poke(&mut self) -> Result<(), PlatformError> {
        match self.strategy {
            ActivityStrategy::ZeroMouseMove => self.fake_relative_motion(0, 0),
            ActivityStrategy::NudgeAndReturn => {
                self.fake_relative_motion(1, 0)?;
                self.fake_relative_motion(-1, 0)
            }
            ActivityStrategy::HarmlessKeyTap => Err(PlatformError::Unsupported(
                "key tap not available on the XTest backend".into(),
            )),
        }
    }
}
