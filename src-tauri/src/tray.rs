//! Tray icon and native menu. Primary interface on Linux, quick access on
//! macOS. State-specific icons land in M3; for now the menu carries state.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
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

    let tray = TrayIconBuilder::with_id("main")
        .icon(
            app.default_window_icon()
                .expect("bundled window icon")
                .clone(),
        )
        .icon_as_template(true)
        .tooltip("EspressoMacchiato — Off")
        .menu(&menu)
        .show_menu_on_left_click(true)
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
        })
        .build(app)?;

    app.manage(TrayHandles {
        tray,
        status_item,
        toggle_item,
    });
    Ok(())
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
