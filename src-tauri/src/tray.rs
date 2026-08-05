//! Tray icon and native menu. Primary interface on Linux, quick access on
//! macOS. State-specific icons land in M3; for now the menu carries state.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
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
    battery_item: MenuItem<tauri::Wry>,
}

/// Tray menu copy in the app language (the native menu is the primary
/// interface on Linux, so it deserves the same localization as the UI).
struct Labels {
    toggle_on: &'static str,
    activate_for: &'static str,
    m15: &'static str,
    h1: &'static str,
    h2: &'static str,
    h4: &'static str,
    end_of_day: &'static str,
    open: &'static str,
    quit: &'static str,
}

fn labels(it: bool) -> Labels {
    if it {
        Labels {
            toggle_on: "Attiva",
            activate_for: "Attiva per…",
            m15: "15 minuti",
            h1: "1 ora",
            h2: "2 ore",
            h4: "4 ore",
            end_of_day: "Fino a fine giornata",
            open: "Apri EspressoMacchiato",
            quit: "Esci",
        }
    } else {
        Labels {
            toggle_on: "Activate",
            activate_for: "Activate for…",
            m15: "15 minutes",
            h1: "1 hour",
            h2: "2 hours",
            h4: "4 hours",
            end_of_day: "Until end of day",
            open: "Open EspressoMacchiato",
            quit: "Quit",
        }
    }
}

/// Timed activation from the tray, no window needed.
fn activate_for(app: &AppHandle, duration_secs: Option<u64>) {
    app.state::<AppState>().engine.set_active(
        true,
        crate::core::state::ActivationReason::Manual,
        duration_secs,
    );
}

fn seconds_until_end_of_day(app: &AppHandle) -> Option<u64> {
    let end_of_day = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .end_of_day
        .clone();
    let target = crate::core::schedule::parse_hhmm(&end_of_day)?;
    Some(crate::core::schedule::seconds_until(
        target,
        chrono::Local::now().naive_local(),
    ))
}

/// Espresso cup glyph drawn at runtime (44×44 RGBA): no icon assets, and
/// the tint can follow the engine state. `None` = solid black for the
/// macOS template icon (the OS recolors it for light/dark menu bars).
fn cup_icon(tint: Option<[u8; 3]>, steam: bool, ring: Option<f64>) -> tauri::image::Image<'static> {
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
    let ring_sdf = |x: f64, y: f64, cx: f64, cy: f64, r_out: f64, r_in: f64| -> f64 {
        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        (d - r_out).max(r_in - d)
    };

    for py in 0..S {
        for px in 0..S {
            let x = px as f64 + 0.5;
            let y = py as f64 + 0.5;

            // Cup body, handle, saucer; optional steam dashes on top.
            let body = rounded_rect(x, y, 19.0, 25.5, 9.0, 7.5, 3.0);
            let handle = ring_sdf(x, y, 30.0, 24.0, 5.0, 2.6).max(19.0 - x);
            let saucer = rounded_rect(x, y, 20.0, 36.5, 12.0, 1.5, 1.5);
            let mut d = body.min(handle).min(saucer);
            if steam {
                let s1 = rounded_rect(x, y, 15.5, 11.0, 1.3, 3.5, 1.3);
                let s2 = rounded_rect(x, y, 22.5, 9.5, 1.3, 3.5, 1.3);
                d = d.min(s1).min(s2);
            }
            // Progress ring around the cup (timer remaining or battery),
            // clockwise from 12 o'clock.
            if let Some(fraction) = ring {
                let cx = 21.0;
                let cy = 22.0;
                let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                let band = (dist - 20.5).max(18.3 - dist);
                if band < 0.5 {
                    let mut angle = (x - cx).atan2(-(y - cy)); // 0 at top, cw
                    if angle < 0.0 {
                        angle += std::f64::consts::TAU;
                    }
                    if angle <= fraction.clamp(0.0, 1.0) * std::f64::consts::TAU {
                        d = d.min(band);
                    }
                }
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
    let it = {
        let state = app.state::<AppState>();
        let lang = state.settings.lock().unwrap().language.clone();
        crate::backend_language(&lang) == "it"
    };
    let l = labels(it);

    let status_item = MenuItem::with_id(app, "status", "Off", false, None::<&str>)?;
    let battery_item = MenuItem::with_id(app, "battery", "…", false, None::<&str>)?;
    let toggle_item = MenuItem::with_id(app, "toggle", l.toggle_on, true, None::<&str>)?;
    let duration_menu = Submenu::with_id_and_items(
        app,
        "activate_for",
        l.activate_for,
        true,
        &[
            &MenuItem::with_id(app, "act_15", l.m15, true, None::<&str>)?,
            &MenuItem::with_id(app, "act_60", l.h1, true, None::<&str>)?,
            &MenuItem::with_id(app, "act_120", l.h2, true, None::<&str>)?,
            &MenuItem::with_id(app, "act_240", l.h4, true, None::<&str>)?,
            &MenuItem::with_id(app, "act_eod", l.end_of_day, true, None::<&str>)?,
        ],
    )?;
    let open_item = MenuItem::with_id(app, "open", l.open, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &battery_item,
            &PredefinedMenuItem::separator(app)?,
            &toggle_item,
            &duration_menu,
            &open_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let builder = TrayIconBuilder::with_id("main")
        .icon(cup_icon(None, false, None))
        .icon_as_template(true)
        .tooltip("EspressoMacchiato — Off")
        .menu(&menu)
        // macOS: left click opens the popover, right click the menu.
        // Linux: the native menu IS the primary interface (spec P3).
        .show_menu_on_left_click(cfg!(not(target_os = "macos")))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => app.state::<AppState>().engine.toggle(),
            "act_15" => activate_for(app, Some(15 * 60)),
            "act_60" => activate_for(app, Some(3600)),
            "act_120" => activate_for(app, Some(2 * 3600)),
            "act_240" => activate_for(app, Some(4 * 3600)),
            "act_eod" => activate_for(app, seconds_until_end_of_day(app)),
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
        battery_item,
    });
    Ok(())
}

/// Battery line inside the tray menu — Linux parity for the popover's
/// battery strip (custom popovers next to the tray are unreliable there).
pub fn set_battery_line(app: &AppHandle, text: &str) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.battery_item.set_text(text);
    }
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

    // Progress ring: timer remaining (when a timed run is active) or
    // battery charge, per the menu_bar_ring setting.
    let ring = {
        let mode = app
            .state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .menu_bar_ring
            .clone();
        match mode.as_str() {
            "timer" => status
                .remaining_secs
                .zip(status.duration_total_secs)
                .filter(|(_, total)| *total > 0)
                .map(|(remaining, total)| remaining as f64 / total as f64),
            "battery" => app
                .try_state::<crate::PowerHub>()
                .and_then(|hub| hub.latest.lock().unwrap().as_ref().and_then(|s| s.percent))
                .map(|p| f64::from(p) / 100.0),
            _ => None,
        }
    };

    // State-colored cup: template (auto light/dark) when off, emerald with
    // steam while running, slate on suspend, red when degraded.
    let (icon, template) = match status.state {
        "active" => (cup_icon(Some(TINT_ACTIVE), true, ring), false),
        "suspended" => (cup_icon(Some(TINT_SUSPENDED), false, ring), false),
        "degraded" => (cup_icon(Some(TINT_DEGRADED), true, ring), false),
        _ => (cup_icon(None, false, None), true),
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
