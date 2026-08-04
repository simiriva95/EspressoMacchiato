mod commands;
mod core;
mod platform;
mod tray;

use std::sync::Mutex;

use tauri::{Emitter, Manager};

use crate::core::engine::{self, EngineHandle, EngineSettings};
use crate::core::events::EngineEvent;
use crate::platform::PreflightFn;

pub struct AppState {
    pub engine: EngineHandle,
    pub preflight: PreflightFn,
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
        .setup(|app| {
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

            let (engine, fut) = engine::start(platform, EngineSettings::default(), sink);
            tauri::async_runtime::spawn(fut);

            app.manage(AppState { engine, preflight });
            tray::create(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Tray app: closing the window hides it, the engine keeps running.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::set_active,
            commands::toggle,
            commands::set_interval_secs,
            commands::set_pause_when_input_recent,
            commands::set_strategy,
            commands::poke_now,
            commands::get_idle_seconds,
            commands::get_permission_status,
            commands::open_permission_settings,
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
