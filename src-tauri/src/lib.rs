mod alerts;
mod commands;
mod config;
mod core;
mod platform;
mod tray;

use std::collections::VecDeque;
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

/// One point in the in-memory metrics history (2 h @ 10 s, never persisted).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PowerSample {
    pub unix_ms: u64,
    pub percent: Option<f32>,
    pub watts: Option<f32>,
    pub temperature_c: Option<f32>,
    pub cpu_percent: f32,
}

/// Latest snapshot + rolling history, shared between the sampler task and
/// the IPC commands.
#[derive(Default)]
pub struct PowerHub {
    pub latest: Mutex<Option<platform::PowerSnapshot>>,
    pub history: Mutex<VecDeque<PowerSample>>,
}

const HISTORY_CAPACITY: usize = 720; // 2 h at one sample every 10 s
const SAMPLE_EVERY: Duration = Duration::from_secs(10);

/// Backend-side language: settings override, else the LANG env var.
fn backend_language(settings_language: &str) -> String {
    match settings_language {
        "it" | "en" => settings_language.to_string(),
        _ => {
            let lang = std::env::var("LANG").unwrap_or_default();
            if lang.to_lowercase().starts_with("it") {
                "it".into()
            } else {
                "en".into()
            }
        }
    }
}

/// "38m · 85%" next to the tray icon, per the configured metrics (max two).
fn compose_menu_text(
    metrics: &[String],
    snapshot: &platform::PowerSnapshot,
    remaining_secs: Option<u64>,
) -> Option<String> {
    let parts: Vec<String> = metrics
        .iter()
        .filter_map(|metric| match metric.as_str() {
            "countdown" => remaining_secs.map(|s| format!("{}m", s.div_ceil(60))),
            "battery" => snapshot.percent.map(|p| format!("{}%", p.round() as i64)),
            "watts" => snapshot.watts.map(|w| format!("{w:.0}W")),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
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
            // Menu bar app: no Dock icon, no Cmd+Tab entry. The tray is home.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_path = config::settings_path()
                .ok_or("cannot resolve the configuration directory")?;
            let (settings, outcome) = config::load(&settings_path);
            let saver = config::spawn_saver(settings_path);

            let mut platform = platform::current_platform();
            let preflight = platform.preflight.clone();
            // The engine never touches power; the sampler task owns it.
            let power_monitor = std::mem::replace(
                &mut platform.power,
                Box::new(platform::NullPowerMonitor),
            );

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
                    // Opt-in "espresso timer expired" alert.
                    if status.state == "off" && status.state_detail == "TimerExpired" {
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            let (enabled, lang) = {
                                let s = state.settings.lock().unwrap();
                                (s.alerts.timer_expired, backend_language(&s.language))
                            };
                            if enabled {
                                let (title, body) = alerts::timer_expired_message(&lang);
                                tray::notify(&app_handle, &title, &body);
                            }
                        }
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
            app.manage(PowerHub::default());
            tray::create(app.handle())?;

            // Power sampler: 10 s cadence, in-memory 2 h history, alert
            // evaluation, optional macOS menu bar text. Nothing persisted.
            let sampler_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut alert_engine = alerts::AlertEngine::new();
                let mut tick = tokio::time::interval(SAMPLE_EVERY);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tick.tick().await;
                    let snapshot = match power_monitor.snapshot() {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::debug!("power snapshot unavailable: {e}");
                            continue;
                        }
                    };
                    let state = sampler_handle.state::<AppState>();
                    let hub = sampler_handle.state::<PowerHub>();

                    let sample = PowerSample {
                        unix_ms: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0),
                        percent: snapshot.percent,
                        watts: snapshot.watts,
                        temperature_c: snapshot.temperature_c,
                        cpu_percent: snapshot.cpu_percent,
                    };
                    {
                        let mut history = hub.history.lock().unwrap();
                        history.push_back(sample.clone());
                        while history.len() > HISTORY_CAPACITY {
                            history.pop_front();
                        }
                    }
                    *hub.latest.lock().unwrap() = Some(snapshot.clone());
                    let _ = sampler_handle.emit("power://sample", &sample);

                    let (alerts_cfg, lang, metrics) = {
                        let s = state.settings.lock().unwrap();
                        (
                            s.alerts.clone(),
                            backend_language(&s.language),
                            s.menu_bar_metrics.clone(),
                        )
                    };
                    for alert in alert_engine.evaluate(
                        &alerts_cfg,
                        &snapshot,
                        &lang,
                        std::time::Instant::now(),
                    ) {
                        tray::notify(&sampler_handle, &alert.title, &alert.body);
                    }

                    #[cfg(target_os = "macos")]
                    {
                        let text = if metrics.is_empty() {
                            None
                        } else {
                            let remaining = state
                                .engine
                                .status()
                                .await
                                .ok()
                                .and_then(|s| s.remaining_secs);
                            compose_menu_text(&metrics, &snapshot, remaining)
                        };
                        tray::set_menu_bar_text(&sampler_handle, text);
                    }
                    #[cfg(not(target_os = "macos"))]
                    let _ = metrics;
                }
            });

            // Quick panel anchored to the tray icon (macOS only; on Linux
            // the native menu is the primary interface).
            #[cfg(target_os = "macos")]
            tauri::WebviewWindowBuilder::new(
                app,
                "popover",
                tauri::WebviewUrl::default(),
            )
            .title("EspressoMacchiato")
            .inner_size(360.0, 500.0)
            .decorations(false)
            .resizable(false)
            .visible(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .transparent(true)
            .effects(tauri::utils::config::WindowEffectsConfig {
                effects: vec![tauri::utils::WindowEffect::HudWindow],
                state: None,
                radius: Some(16.0),
                color: None,
            })
            .build()?;

            // First run with something to configure: surface the window.
            let show_main = {
                let state = app.state::<AppState>();
                let s = state.settings.lock().unwrap();
                !s.onboarding_done
            };
            if show_main {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

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
            commands::get_power,
            commands::get_power_history,
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
