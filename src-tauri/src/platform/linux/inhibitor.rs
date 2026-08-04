//! Sleep/idle inhibition via D-Bus, with graceful degradation:
//! 1. logind `Inhibit("idle:sleep", …, "block")` — returns an fd; the
//!    inhibition lives while the fd is open.
//! 2. `org.freedesktop.ScreenSaver.Inhibit` — cookie-based (GNOME/KDE/XFCE).
//! 3. `org.gnome.SessionManager.Inhibit` with the idle flag, GNOME only.
//!
//! Any subset that succeeds is held; if none does, acquire fails and the
//! engine reports Degraded naming the detected desktop.

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedFd;

use super::session;
use crate::platform::{InhibitOptions, PlatformError, SleepInhibitor};

const APP_NAME: &str = "EspressoMacchiato";
const REASON: &str = "Keeping the machine awake and present";

/// org.gnome.SessionManager.Inhibit flag: "Inhibit the session being marked
/// as idle".
const GNOME_INHIBIT_IDLE: u32 = 8;

#[derive(Default)]
pub struct LinuxInhibitor {
    /// logind: inhibition lives while this fd is open.
    logind_fd: Option<OwnedFd>,
    /// (connection, cookie) pairs for cookie-based interfaces; the
    /// connection must stay alive to UnInhibit later.
    screensaver: Option<(Connection, u32)>,
    gnome_session: Option<(Connection, u32)>,
}

fn logind_inhibit() -> Result<OwnedFd, PlatformError> {
    let conn =
        Connection::system().map_err(|e| PlatformError::Other(format!("system bus: {e}")))?;
    let proxy = Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .map_err(|e| PlatformError::Other(format!("login1 proxy: {e}")))?;
    let fd: OwnedFd = proxy
        .call("Inhibit", &("idle:sleep", APP_NAME, REASON, "block"))
        .map_err(|e| PlatformError::Other(format!("login1 Inhibit: {e}")))?;
    Ok(fd)
}

fn screensaver_inhibit() -> Result<(Connection, u32), PlatformError> {
    let conn =
        Connection::session().map_err(|e| PlatformError::Other(format!("session bus: {e}")))?;
    let proxy = Proxy::new(
        &conn,
        "org.freedesktop.ScreenSaver",
        "/org/freedesktop/ScreenSaver",
        "org.freedesktop.ScreenSaver",
    )
    .map_err(|e| PlatformError::Other(format!("ScreenSaver proxy: {e}")))?;
    let cookie: u32 = proxy
        .call("Inhibit", &(APP_NAME, REASON))
        .map_err(|e| PlatformError::Other(format!("ScreenSaver Inhibit: {e}")))?;
    Ok((conn, cookie))
}

fn gnome_session_inhibit() -> Result<(Connection, u32), PlatformError> {
    let conn =
        Connection::session().map_err(|e| PlatformError::Other(format!("session bus: {e}")))?;
    let proxy = Proxy::new(
        &conn,
        "org.gnome.SessionManager",
        "/org/gnome/SessionManager",
        "org.gnome.SessionManager",
    )
    .map_err(|e| PlatformError::Other(format!("SessionManager proxy: {e}")))?;
    let cookie: u32 = proxy
        .call("Inhibit", &(APP_NAME, 0u32, REASON, GNOME_INHIBIT_IDLE))
        .map_err(|e| PlatformError::Other(format!("SessionManager Inhibit: {e}")))?;
    Ok((conn, cookie))
}

impl SleepInhibitor for LinuxInhibitor {
    fn acquire(&mut self, _opts: InhibitOptions) -> Result<(), PlatformError> {
        if self.is_active() {
            return Ok(()); // idempotent, never stack fds/cookies
        }

        let mut errors: Vec<String> = Vec::new();

        match logind_inhibit() {
            Ok(fd) => self.logind_fd = Some(fd),
            Err(e) => errors.push(e.to_string()),
        }
        match screensaver_inhibit() {
            Ok(pair) => self.screensaver = Some(pair),
            Err(e) => errors.push(e.to_string()),
        }
        let session = session::detect();
        if session.desktop.to_lowercase().contains("gnome") {
            match gnome_session_inhibit() {
                Ok(pair) => self.gnome_session = Some(pair),
                Err(e) => errors.push(e.to_string()),
            }
        }

        if self.is_active() {
            Ok(())
        } else {
            Err(PlatformError::Unsupported(format!(
                "no inhibition interface answered on {} ({:?} session): {}",
                session.desktop,
                session.session_type,
                errors.join("; ")
            )))
        }
    }

    fn release(&mut self) -> Result<(), PlatformError> {
        // Dropping the fd ends the logind inhibition.
        self.logind_fd = None;

        if let Some((conn, cookie)) = self.screensaver.take() {
            if let Ok(proxy) = Proxy::new(
                &conn,
                "org.freedesktop.ScreenSaver",
                "/org/freedesktop/ScreenSaver",
                "org.freedesktop.ScreenSaver",
            ) {
                let _: Result<(), _> = proxy.call("UnInhibit", &(cookie,));
            }
        }
        if let Some((conn, cookie)) = self.gnome_session.take() {
            if let Ok(proxy) = Proxy::new(
                &conn,
                "org.gnome.SessionManager",
                "/org/gnome/SessionManager",
                "org.gnome.SessionManager",
            ) {
                let _: Result<(), _> = proxy.call("Uninhibit", &(cookie,));
            }
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.logind_fd.is_some() || self.screensaver.is_some() || self.gnome_session.is_some()
    }
}

impl Drop for LinuxInhibitor {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
