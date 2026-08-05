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

/// Espresso cup glyph drawn at runtime (44×44 RGBA): no icon assets, and
/// the tint can follow the engine state. `None` = solid black for the
/// macOS template icon (the OS recolors it for light/dark menu bars).
fn cup_icon(tint: Option<[u8; 3]>, steam: bool) -> tauri::image::Image<'static> {
    const S: usize = 44;
    let [r, g, b] = tint.unwrap_or([0, 0, 0]);
    let mut rgba = vec![0u8; S * S * 4];

    // Signed-distance helpers, all in pixel space.
    let rounded_rect = |x: f64, y: f64, cx: f64, cy: f64, hw: f64, hh: f64, rad: f64| -> f64 {
        let dx = (x - cx).abs() - (hw - rad);
        let dy = (y - cy).abs() - (hh - rad);
        let ox = dx.max(0.0);
        let oy = dy.max(0.0);
        (ox * ox + oy * oy).sqrt() + dx.max(dy).min(0.0) - rad
    };
    let ring = |x: f64, y: f64, cx: f64, cy: f64, r_out: f64, r_in: f64| -> f64 {
        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        (d - r_out).max(r_in - d)
    };

    for py in 0..S {
        for px in 0..S {
            let x = px as f64 + 0.5;
            let y = py as f64 + 0.5;

            // Cup body, handle, saucer; optional steam dashes on top.
            let body = rounded_rect(x, y, 19.0, 25.5, 9.0, 7.5, 3.0);
            let handle = ring(x, y, 30.0, 24.0, 5.0, 2.6).max(19.0 - x);
            let saucer = rounded_rect(x, y, 20.0, 36.5, 12.0, 1.5, 1.5);
            let mut d = body.min(handle).min(saucer);
            if steam {
                let s1 = rounded_rect(x, y, 15.5, 11.0, 1.3, 3.5, 1.3);
                let s2 = rounded_rect(x, y, 22.5, 9.5, 1.3, 3.5, 1.3);
                d = d.min(s1).min(s2);
            }

            // 1px anti-aliased edge.
            let alpha = (0.5 - d).clamp(0.0, 1.0);
            if alpha > 0.0 {
                let i = (py * S + px) * 4;
                rgba[i] = r;
                rgba[i + 1] = g;
                rgba[i + 2] = b;
                rgba[i + 3] = (alpha * 255.0) as u8;
            }
        }
    }
    tauri::image::Image::new_owned(rgba, S as u32, S as u32)
}

const TINT_ACTIVE: [u8; 3] = [52, 211, 153]; // emerald
const TINT_SUSPENDED: [u8; 3] = [148, 163, 184]; // slate
const TINT_DEGRADED: [u8; 3] = [248, 113, 113]; // soft red

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
        .icon(cup_icon(None, false))
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

    // State-colored cup: template (auto light/dark) when off, emerald with
    // steam while running, slate on suspend, red when degraded.
    let (icon, template) = match status.state {
        "active" => (cup_icon(Some(TINT_ACTIVE), true), false),
        "suspended" => (cup_icon(Some(TINT_SUSPENDED), false), false),
        "degraded" => (cup_icon(Some(TINT_DEGRADED), true), false),
        _ => (cup_icon(None, false), true),
    };
    let _ = handles.tray.set_icon(Some(icon));
    let _ = handles.tray.set_icon_as_template(template);
}

/// Compact text next to the tray icon (macOS menu bar only).
#[cfg(target_os = "macos")]
pub fn set_menu_bar_text(app: &AppHandle, text: Option<String>) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.tray.set_title(text);
    }
}

/// Plain notification helper (alerts, timer expiry).
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
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
