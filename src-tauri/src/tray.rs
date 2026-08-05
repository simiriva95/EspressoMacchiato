//! Tray icon and native menu. Primary interface on Linux, quick access on
//! macOS. State-specific icons land in M3; for now the menu carries state.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
#[cfg(target_os = "macos")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use std::sync::Mutex;

use crate::core::events::StatusSnapshot;
use crate::AppState;

/// What the current icon frame should look like — updated by `sync`, read by
/// the animation task so it can redraw steam frames without an engine query.
#[derive(Clone, Default)]
struct TrayVisual {
    tint: Option<[u8; 3]>,
    steam: bool,
    ring: Option<f64>,
    template: bool,
    /// Animate (redraw steam frames) only while the engine is active.
    animate: bool,
}

pub struct TrayHandles {
    tray: tauri::tray::TrayIcon,
    status_item: MenuItem<tauri::Wry>,
    toggle_item: MenuItem<tauri::Wry>,
    battery_item: MenuItem<tauri::Wry>,
    visual: Mutex<TrayVisual>,
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

/// Espresso cup glyph drawn at runtime (super-sampled 4× to 44×44 RGBA): no
/// icon assets, tint follows the engine state, optional progress arc around
/// the cup. `None` tint = solid black for the macOS template icon (the OS
/// recolors it for light/dark menu bars).
fn cup_icon(tint: Option<[u8; 3]>, steam: bool, ring: Option<f64>) -> tauri::image::Image<'static> {
    cup_icon_phase(tint, steam, ring, 0.0)
}

/// `phase` in 0..1 drives the steam animation: two curls rise and fade on a
/// loop. 0 = static frame (used everywhere except the animation task).
fn cup_icon_phase(
    tint: Option<[u8; 3]>,
    steam: bool,
    ring: Option<f64>,
    phase: f64,
) -> tauri::image::Image<'static> {
    const S: usize = 44;
    const SS: usize = 4; // supersampling factor for crisp curves
    let [r, g, b] = tint.unwrap_or([0, 0, 0]);
    let mut rgba = vec![0u8; S * S * 4];

    // Everything is drawn on a 44-unit canvas centered at (22,22).
    let cx = 22.0;
    let rounded_rect = |x: f64, y: f64, cx: f64, cy: f64, hw: f64, hh: f64, rad: f64| -> f64 {
        let dx = (x - cx).abs() - (hw - rad);
        let dy = (y - cy).abs() - (hh - rad);
        (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() + dx.max(dy).min(0.0) - rad
    };

    // A big, legible cup: tapered body (wider at the rim), a hollow so it
    // reads as a mug, a handle ring on the right, a saucer line, and two
    // steam curls when active. When a ring is drawn the cup shrinks a touch
    // to leave room; otherwise it fills the canvas.
    let scale = if ring.is_some() { 0.82 } else { 1.0 };
    let sx0 = |x: f64| cx + (x - cx) / scale;
    let coverage = |px: f64, py: f64| -> f64 {
        let x = sx0(px);
        let y = cx + (py - cx) / scale;
        // Rim (top) wider than base: blend two rounded rects.
        let rim = rounded_rect(x, y, cx - 1.0, 16.5, 11.5, 3.0, 2.5);
        let body = rounded_rect(x, y, cx - 1.5, 23.5, 9.5, 8.5, 3.5);
        let mut d = rim.min(body);
        // Hollow the coffee surface so it looks like a cup, not a blob.
        let hollow = rounded_rect(x, y, cx - 1.0, 15.0, 8.5, 1.6, 1.4);
        d = d.max(-hollow);
        // Handle.
        let handle = {
            let dd = ((x - (cx + 11.0)).powi(2) + (y - 22.0).powi(2)).sqrt();
            (dd - 6.0).max(3.3 - dd)
        };
        d = d.min(handle);
        // Saucer.
        let saucer = rounded_rect(x, y, cx - 1.0, 35.5, 13.0, 1.6, 1.6);
        d = d.min(saucer);
        (0.5 - d).clamp(0.0, 1.0)
    };

    // Steam: two curls that rise and fade on a loop, offset in time. Drawn
    // separately from the cup so their alpha can animate. `phase` 0..1.
    let steam_alpha = |px: f64, py: f64| -> f64 {
        if !steam {
            return 0.0;
        }
        let curl = |cx_s: f64, p: f64| -> f64 {
            let local = (phase + p).fract(); // 0..1 progress up
            let x = sx0(px);
            let y = cx + (py - cx) / scale;
            // Rises from y≈13 to y≈3, gentle horizontal sway.
            let base_y = 13.0 - local * 10.0;
            let sway = (local * std::f64::consts::TAU).sin() * 1.3;
            let d = rounded_rect(x, y, cx_s + sway, base_y, 1.1, 2.6, 1.1);
            let cov = (0.5 - d).clamp(0.0, 1.0);
            // Fade in at the start, out toward the top.
            let fade = (local * 4.0).min(1.0) * (1.0 - local).powf(0.7);
            cov * fade
        };
        curl(cx - 4.5, 0.0).max(curl(cx + 2.0, 0.5))
    };

    // Progress arc: stroked circle from 12 o'clock, clockwise, drawn for the
    // covered fraction. A full circle (fraction 1.0) reads as "active,
    // indefinitely" — always something to see when on.
    let arc = |x: f64, y: f64, fraction: f64| -> f64 {
        let dx = x - cx;
        let dy = y - cx;
        let dist = (dx * dx + dy * dy).sqrt();
        let band = 1.6 - (dist - 20.4).abs(); // ring at radius ~20.4, ~3.2 wide
        if band <= 0.0 {
            return 0.0;
        }
        let mut angle = dx.atan2(-dy); // 0 at top, clockwise
        if angle < 0.0 {
            angle += std::f64::consts::TAU;
        }
        if angle <= fraction.clamp(0.0, 1.0) * std::f64::consts::TAU {
            band.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    for py in 0..S {
        for px in 0..S {
            let mut acc = 0.0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f64 + (sx as f64 + 0.5) / SS as f64;
                    let y = py as f64 + (sy as f64 + 0.5) / SS as f64;
                    let mut c = coverage(x, y);
                    if let Some(fraction) = ring {
                        c = c.max(arc(x, y, fraction));
                    }
                    c = c.max(steam_alpha(x, y));
                    acc += c;
                }
            }
            let alpha = acc / (SS * SS) as f64;
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
        // macOS: left click opens the dashboard, right click the menu.
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
            ..
        } = event
        {
            toggle_dashboard(tray.app_handle());
        }
    });

    let tray = builder.build(app)?;

    app.manage(TrayHandles {
        tray,
        status_item,
        toggle_item,
        battery_item,
        visual: Mutex::new(TrayVisual {
            template: true,
            ..Default::default()
        }),
    });

    // Steam animation: ~10 fps while the engine is active, idle otherwise.
    let anim_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut phase = 0.0_f64;
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(90));
        loop {
            ticker.tick().await;
            let Some(handles) = anim_handle.try_state::<TrayHandles>() else {
                continue;
            };
            let visual = handles.visual.lock().unwrap().clone();
            if !visual.animate {
                continue; // static frame already set by sync()
            }
            phase = (phase + 0.045).fract();
            let icon = cup_icon_phase(visual.tint, visual.steam, visual.ring, phase);
            let _ = handles.tray.set_icon(Some(icon));
            let _ = handles.tray.set_icon_as_template(visual.template);
        }
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

/// Left-click on the tray: show and focus the dashboard (or hide it if it's
/// already the frontmost window).
#[cfg(target_os = "macos")]
fn toggle_dashboard(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    if visible && focused {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
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

    // Progress ring. When the engine is on there is ALWAYS a ring: a timed
    // run drains it, battery mode tracks charge, and an indefinite /
    // scheduled run shows a full circle — so "active" is always legible in
    // the menu bar, not just when a countdown happens to be running.
    let on = status.state != "off";
    let ring = if !on {
        None
    } else {
        let mode = app
            .state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .menu_bar_ring
            .clone();
        let fraction = match mode.as_str() {
            "off" => None,
            "battery" => app
                .try_state::<crate::PowerHub>()
                .and_then(|hub| hub.latest.lock().unwrap().as_ref().and_then(|s| s.percent))
                .map(|p| f64::from(p) / 100.0),
            // "timer" (default): countdown fraction if a timed run, else full.
            _ => status
                .remaining_secs
                .zip(status.duration_total_secs)
                .filter(|(_, total)| *total > 0)
                .map(|(remaining, total)| remaining as f64 / total as f64)
                .or(Some(1.0)),
        };
        // "off" mode means no ring at all; every other mode shows at least a
        // full circle while on.
        if mode == "off" {
            None
        } else {
            fraction.or(Some(1.0))
        }
    };

    // State-colored cup: template (auto light/dark) when off, emerald with
    // steam while running, slate on suspend, red when degraded. Steam only
    // animates while active/degraded (a live episode).
    let (tint, steam, template) = match status.state {
        "active" => (Some(TINT_ACTIVE), true, false),
        "suspended" => (Some(TINT_SUSPENDED), false, false),
        "degraded" => (Some(TINT_DEGRADED), true, false),
        _ => (None, false, true),
    };
    let ring = if status.state == "off" { None } else { ring };
    let animate = steam;

    // Record the frame for the animation task, and paint one now so a
    // non-animated state (off/suspended) updates immediately.
    *handles.visual.lock().unwrap() = TrayVisual {
        tint,
        steam,
        ring,
        template,
        animate,
    };
    if !animate {
        let _ = handles.tray.set_icon(Some(cup_icon(tint, steam, ring)));
        let _ = handles.tray.set_icon_as_template(template);
    }
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
