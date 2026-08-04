//! Tray icon and native menu. Primary interface on Linux, quick access on
//! macOS. State-specific icons land in M3; for now the menu carries state.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
#[cfg(target_os = "macos")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::core::events::StatusSnapshot;
use crate::AppState;

pub struct TrayHandles {
    tray: tauri::tray::TrayIcon,
    status_item: MenuItem<tauri::Wry>,
    toggle_item: MenuItem<tauri::Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let status_item = MenuItem::with_id(app, "status", "Off", false, None::<&str>)?;
    let toggle_item = MenuItem::with_id(app, "toggle", "Activate", true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Open EspressoMacchiato", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &PredefinedMenuItem::separator(app)?,
            &toggle_item,
            &open_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let builder = TrayIconBuilder::with_id("main")
        .icon(
            app.default_window_icon()
                .expect("bundled window icon")
                .clone(),
        )
        .icon_as_template(true)
        .tooltip("EspressoMacchiato — Off")
        .menu(&menu)
        // macOS: left click opens the popover, right click the menu.
        // Linux: the native menu IS the primary interface (spec P3).
        .show_menu_on_left_click(cfg!(not(target_os = "macos")))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => app.state::<AppState>().engine.toggle(),
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.state::<AppState>().engine.shutdown();
                // Give the engine actor a beat to release the inhibitor
                // deterministically before the process goes away.
                std::thread::sleep(std::time::Duration::from_millis(150));
                app.exit(0);
            }
            _ => {}
        });

    #[cfg(target_os = "macos")]
    let builder = builder.on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            rect,
            ..
        } = event
        {
            toggle_popover(tray.app_handle(), rect);
        }
    });

    let tray = builder.build(app)?;

    app.manage(TrayHandles {
        tray,
        status_item,
        toggle_item,
    });
    Ok(())
}

/// Show the quick popover anchored under the tray icon, or hide it if
/// already visible. macOS only: popover placement next to a tray icon is
/// not reliable on Linux (spec P3), where the native menu is primary.
#[cfg(target_os = "macos")]
fn toggle_popover(app: &AppHandle, rect: tauri::Rect) {
    let Some(popover) = app.get_webview_window("popover") else {
        return;
    };
    if popover.is_visible().unwrap_or(false) {
        let _ = popover.hide();
        return;
    }
    let scale = popover.scale_factor().unwrap_or(2.0);
    let (icon_x, icon_y, icon_w, icon_h) = physical_rect(&rect, scale);
    let width = popover
        .outer_size()
        .map(|s| f64::from(s.width))
        .unwrap_or(360.0 * scale);
    let x = icon_x + icon_w / 2.0 - width / 2.0;
    let y = icon_y + icon_h + 4.0 * scale;
    let _ = popover.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = popover.show();
    let _ = popover.set_focus();
}

#[cfg(target_os = "macos")]
fn physical_rect(rect: &tauri::Rect, scale: f64) -> (f64, f64, f64, f64) {
    let (x, y) = match rect.position {
        tauri::Position::Physical(p) => (f64::from(p.x), f64::from(p.y)),
        tauri::Position::Logical(p) => (p.x * scale, p.y * scale),
    };
    let (w, h) = match rect.size {
        tauri::Size::Physical(s) => (f64::from(s.width), f64::from(s.height)),
        tauri::Size::Logical(s) => (s.width * scale, s.height * scale),
    };
    (x, y, w, h)
}

/// Reflect engine state in the tray menu and tooltip.
pub fn sync(app: &AppHandle, status: &StatusSnapshot) {
    let Some(handles) = app.try_state::<TrayHandles>() else {
        return; // engine event before the tray exists (startup)
    };
    let (label, toggle) = match status.state {
        "active" => ("Active", "Deactivate"),
        "suspended" => ("Suspended", "Deactivate"),
        "degraded" => ("Active (degraded — check permissions)", "Deactivate"),
        _ => ("Off", "Activate"),
    };
    let _ = handles.status_item.set_text(label);
    let _ = handles.toggle_item.set_text(toggle);
    let _ = handles
        .tray
        .set_tooltip(Some(format!("EspressoMacchiato — {label}")));
}

/// One actionable notification when entering Degraded (deduped by caller).
pub fn notify_degraded(app: &AppHandle, status: &StatusSnapshot) {
    let body = status
        .degradations
        .first()
        .map(|d| {
            let mut text = d.detail.clone();
            if let Some(help) = &d.help {
                text.push('\n');
                text.push_str(help);
            }
            text
        })
        .unwrap_or_else(|| status.state_detail.clone());
    let _ = app
        .notification()
        .builder()
        .title("EspressoMacchiato can't reset the idle counter")
        .body(body)
        .show();
}
