//! Idle time, tried in cascade; the chosen backend is reported as `source`
//! so the Idle Monitor can show where the number comes from.

use zbus::blocking::{Connection, Proxy};

use x11rb::connection::Connection as _;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::platform::{IdleReader, PlatformError};

#[allow(clippy::large_enum_variant)] // exactly one Backend exists per process
enum Backend {
    XScreenSaver { conn: RustConnection, root: u32 },
    Mutter(Connection),
    KdeScreenSaver(Connection),
}

pub struct LinuxIdleReader {
    backend: Backend,
}

impl LinuxIdleReader {
    pub fn detect() -> Result<Self, PlatformError> {
        // X11 first: cheapest and most precise where available.
        if let Ok((conn, screen_num)) = x11rb::connect(None) {
            let root = conn.setup().roots[screen_num].root;
            // Probe once: the extension may be missing.
            if conn
                .screensaver_query_info(root)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some()
            {
                return Ok(Self {
                    backend: Backend::XScreenSaver { conn, root },
                });
            }
        }
        if let Ok(conn) = Connection::session() {
            if mutter_idle_ms(&conn).is_ok() {
                return Ok(Self {
                    backend: Backend::Mutter(conn),
                });
            }
        }
        if let Ok(conn) = Connection::session() {
            if kde_idle_secs(&conn).is_ok() {
                return Ok(Self {
                    backend: Backend::KdeScreenSaver(conn),
                });
            }
        }
        Err(PlatformError::Unsupported(
            "no idle source: XScreenSaver, Mutter IdleMonitor and org.freedesktop.ScreenSaver all unavailable".into(),
        ))
    }
}

fn mutter_idle_ms(conn: &Connection) -> Result<u64, PlatformError> {
    let proxy = Proxy::new(
        conn,
        "org.gnome.Mutter.IdleMonitor",
        "/org/gnome/Mutter/IdleMonitor/Core",
        "org.gnome.Mutter.IdleMonitor",
    )
    .map_err(|e| PlatformError::Other(format!("Mutter proxy: {e}")))?;
    proxy
        .call("GetIdletime", &())
        .map_err(|e| PlatformError::Other(format!("Mutter GetIdletime: {e}")))
}

fn kde_idle_secs(conn: &Connection) -> Result<u32, PlatformError> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.ScreenSaver",
        "/org/freedesktop/ScreenSaver",
        "org.freedesktop.ScreenSaver",
    )
    .map_err(|e| PlatformError::Other(format!("ScreenSaver proxy: {e}")))?;
    proxy
        .call("GetSessionIdleTime", &())
        .map_err(|e| PlatformError::Other(format!("GetSessionIdleTime: {e}")))
}

impl IdleReader for LinuxIdleReader {
    fn idle_seconds(&self) -> Result<f64, PlatformError> {
        match &self.backend {
            Backend::XScreenSaver { conn, root } => {
                let info = conn
                    .screensaver_query_info(*root)
                    .map_err(|e| PlatformError::Other(format!("XScreenSaver query: {e}")))?
                    .reply()
                    .map_err(|e| PlatformError::Other(format!("XScreenSaver reply: {e}")))?;
                Ok(f64::from(info.ms_since_user_input) / 1000.0)
            }
            Backend::Mutter(conn) => Ok(mutter_idle_ms(conn)? as f64 / 1000.0),
            Backend::KdeScreenSaver(conn) => Ok(f64::from(kde_idle_secs(conn)?)),
        }
    }

    fn source(&self) -> &'static str {
        match self.backend {
            Backend::XScreenSaver { .. } => "XScreenSaver",
            Backend::Mutter(_) => "Mutter",
            Backend::KdeScreenSaver(_) => "org.freedesktop.ScreenSaver",
        }
    }
}

/// Placeholder when nothing answers: the UI shows "not detectable on this
/// compositor" instead of breaking.
pub struct UnavailableIdleReader(pub String);

impl IdleReader for UnavailableIdleReader {
    fn idle_seconds(&self) -> Result<f64, PlatformError> {
        Err(PlatformError::Unsupported(self.0.clone()))
    }

    fn source(&self) -> &'static str {
        "unavailable"
    }
}
