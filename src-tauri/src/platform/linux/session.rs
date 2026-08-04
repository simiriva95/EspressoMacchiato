//! Detect X11 vs Wayland and the desktop environment. Half of Linux bug
//! reports resolve themselves once this is known, so it is surfaced in
//! diagnostics and in degradation messages.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_type: SessionType,
    pub desktop: String,
}

pub fn detect() -> SessionInfo {
    let session_type = match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("wayland") => SessionType::Wayland,
        Ok("x11") => SessionType::X11,
        _ => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                SessionType::Wayland
            } else if std::env::var("DISPLAY").is_ok() {
                SessionType::X11
            } else {
                SessionType::Unknown
            }
        }
    };
    SessionInfo {
        session_type,
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into()),
    }
}

/// True when an X server (or XWayland) is reachable.
pub fn x11_reachable() -> bool {
    std::env::var("DISPLAY").is_ok()
}
