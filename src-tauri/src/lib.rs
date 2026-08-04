mod commands;
mod config;
mod core;
mod platform;
mod tray;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

use crate::config::{LoadOutcome, Settings};
use crate::core::engine::{self, clamp_interval_secs, EngineHandle, EngineSettings};
use crate::core::events::EngineEvent;
use crate::core::state::ActivationReason;
use crate::platform::PreflightFn;

pub struct AppState {
    pub engine: EngineHandle,
    pub preflight: PreflightFn,
    pub settings: Mutex<Settings>,
    pub saver: tokio::sync::mpsc::UnboundedSender<Settings>,
}

/// (Re)register the global toggle hotkey. Empty string = no hotkey.
pub(crate) fn register_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts.unregister_all().map_err(|e| e.to_string())?;
    if hotkey.trim().is_empty() {
        return Ok(());
    }
    shortcuts
        .on_shortcut(hotkey, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                app.state::<AppState>().engine.toggle();
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second launch: surface the existing window instead.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let settings_path = config::settings_path()
                .ok_or("cannot resolve the configuration directory")?;
            let (settings, outcome) = config::load(&settings_path);
            let saver = config::spawn_saver(settings_path);

            let platform = platform::current_platform();
            let preflight = platform.preflight.clone();

            let app_handle = app.handle().clone();
            // One actionable notification per degradation episode, not spam.
            let degraded_notified = Mutex::new(false);
            let sink = Box::new(move |event: EngineEvent| {
                if let EngineEvent::StateChanged { status } = &event {
                    tray::sync(&app_handle, status);
                    let mut notified = degraded_notified.lock().unwrap();
                    match status.state {
                        "degraded" if !*notified => {
                            *notified = true;
                            tray::notify_degraded(&app_handle, status);
                        }
                        "active" | "off" => *notified = false,
                        _ => {}
                    }
                }
                let _ = app_handle.emit("engine://event", &event);
            });

            let engine_settings = EngineSettings {
                interval: Duration::from_secs(clamp_interval_secs(settings.interval_secs)),
                schedule: settings.schedule.clone(),
                conditions: settings.conditions.clone(),
            };
            let (engine, fut) = engine::start(platform, engine_settings, sink);
            tauri::async_runtime::spawn(fut);
            engine.set_strategy(settings.strategy);
            if settings.activate_on_start {
                engine.set_active(true, ActivationReason::Autostart, None);
            }

            if let Err(e) = register_hotkey(app.handle(), &settings.hotkey) {
                tracing::warn!("hotkey '{}' not registered: {e}", settings.hotkey);
            }
            // Converge the OS login item with the stored preference.
            let autolaunch = app.autolaunch();
            let result = if settings.autostart {
                autolaunch.enable()
            } else {
                autolaunch.disable()
            };
            if let Err(e) = result {
                tracing::warn!("autostart sync failed: {e}");
            }

            if let LoadOutcome::CorruptRecovered(quarantine) = outcome {
                let _ = app
                    .notification()
                    .builder()
                    .title("Settings were reset")
                    .body(format!(
                        "settings.json was unreadable and has been moved to {}. Defaults are in effect.",
                        quarantine.display()
                    ))
                    .show();
            }

            app.manage(AppState {
                engine,
                preflight,
                settings: Mutex::new(settings),
                saver,
            });
            tray::create(app.handle())?;

            // Quick panel anchored to the tray icon (macOS only; on Linux
            // the native menu is the primary interface).
            #[cfg(target_os = "macos")]
            tauri::WebviewWindowBuilder::new(
                app,
                "popover",
                tauri::WebviewUrl::default(),
            )
            .title("EspressoMacchiato")
            .inner_size(360.0, 480.0)
            .decorations(false)
            .resizable(false)
            .visible(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                // Tray app: closing a window hides it, the engine keeps running.
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let _ = window.hide();
                    api.prevent_close();
                }
                // The popover dismisses itself when it loses focus.
                tauri::WindowEvent::Focused(false) if window.label() == "popover" => {
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::set_active,
            commands::toggle,
            commands::poke_now,
            commands::get_idle_seconds,
            commands::get_permission_status,
            commands::open_permission_settings,
            commands::get_settings,
            commands::update_settings,
            commands::open_settings_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Belt and braces: assertions/fds die with the process anyway,
                // but release them deterministically on a clean quit.
                let state = app.state::<AppState>();
                state.engine.shutdown();
            }
        });
}
