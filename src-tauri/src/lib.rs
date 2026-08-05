mod alerts;
mod commands;
mod config;
mod core;
mod platform;
mod sound;
mod stats;
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

const HISTORY_CAPACITY: usize = 2880; // 8 h at one sample every 10 s (RAM only)
const SAMPLE_EVERY: Duration = Duration::from_secs(10);

/// Weekly/daily aggregates (see stats.rs), shared by sink and sampler.
pub struct StatsHub {
    pub stats: Mutex<stats::Stats>,
    pub path: std::path::PathBuf,
}

#[derive(Debug, PartialEq)]
pub(crate) enum DeepAction {
    On(Option<u64>),
    Off,
    Toggle,
}

/// espresso://on[?for=90m|1h|3600], espresso://off, espresso://toggle
pub(crate) fn parse_deep_link(url: &str) -> Option<DeepAction> {
    let rest = url.trim().strip_prefix("espresso://")?.to_lowercase();
    let (action, query) = match rest.split_once('?') {
        Some((a, q)) => (a.trim_matches('/'), Some(q)),
        None => (rest.trim_matches('/'), None),
    };
    match action {
        "off" => Some(DeepAction::Off),
        "toggle" => Some(DeepAction::Toggle),
        "on" => {
            let secs = query
                .and_then(|q| {
                    q.split('&')
                        .find_map(|pair| pair.strip_prefix("for="))
                        .map(str::to_string)
                })
                .and_then(|v| {
                    if let Some(h) = v.strip_suffix('h') {
                        h.parse::<u64>().ok().map(|n| n * 3600)
                    } else if let Some(m) = v.strip_suffix('m') {
                        m.parse::<u64>().ok().map(|n| n * 60)
                    } else {
                        v.trim_end_matches('s').parse::<u64>().ok()
                    }
                });
            Some(DeepAction::On(secs))
        }
        _ => None,
    }
}

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
/// Dynamic by design: countdown only exists while a timed activation runs,
/// battery only speaks up below 20% — the menu bar stays quiet otherwise.
#[cfg(target_os = "macos")]
fn compose_menu_text(
    metrics: &[String],
    snapshot: &platform::PowerSnapshot,
    remaining_secs: Option<u64>,
) -> Option<String> {
    const BATTERY_ATTENTION_PERCENT: f32 = 20.0;
    let parts: Vec<String> = metrics
        .iter()
        .filter_map(|metric| match metric.as_str() {
            "countdown" => remaining_secs.map(|s| format!("{}m", s.div_ceil(60))),
            "battery" => snapshot
                .percent
                .filter(|p| *p <= BATTERY_ATTENTION_PERCENT && !snapshot.on_ac)
                .map(|p| format!("{}%", p.round() as i64)),
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
        .plugin(tauri_plugin_deep_link::init())
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
            // Previous snapshot: drives the activation sound and the
            // espresso-shot counter (completed 25-minute runs).
            let prev_status: Mutex<Option<crate::core::events::StatusSnapshot>> = Mutex::new(None);
            let sink = Box::new(move |event: EngineEvent| {
                if let EngineEvent::StateChanged { status } = &event {
                    tray::sync(&app_handle, status);

                    let prev = prev_status.lock().unwrap().replace(status.clone());
                    let was_off = prev.as_ref().is_none_or(|p| p.state == "off");
                    if status.state == "active" && was_off {
                        let sound_on = app_handle
                            .try_state::<AppState>()
                            .map(|s| s.settings.lock().unwrap().sound_on_activate)
                            .unwrap_or(false);
                        if sound_on {
                            sound::play_steam();
                        }
                    }
                    if status.state == "off"
                        && status.state_detail == "TimerExpired"
                        && prev.as_ref().and_then(|p| p.duration_total_secs)
                            == Some(stats::SHOT_SECS)
                    {
                        if let Some(hub) = app_handle.try_state::<StatsHub>() {
                            let mut st = hub.stats.lock().unwrap();
                            st.rollover(chrono::Local::now().date_naive());
                            st.shots_today += 1;
                            stats::save(&hub.path, &st);
                        }
                    }

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
            // Missing Accessibility at launch → show the system prompt once,
            // which also puts the app in the Privacy & Security list (with
            // the silent check alone there is nothing for the user to find).
            #[cfg(target_os = "macos")]
            if !platform::macos::permissions::accessibility_trusted() {
                platform::macos::permissions::request_accessibility();
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
            {
                let path = stats::stats_path().ok_or("cannot resolve the stats path")?;
                let mut loaded = stats::load(&path);
                loaded.rollover(chrono::Local::now().date_naive());
                app.manage(StatsHub {
                    stats: Mutex::new(loaded),
                    path,
                });
            }
            tray::create(app.handle())?;

            // espresso://on|off|toggle — Shortcuts, Raycast, scripts.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        match parse_deep_link(url.as_str()) {
                            Some(DeepAction::On(secs)) => {
                                handle.state::<AppState>().engine.set_active(
                                    true,
                                    ActivationReason::Manual,
                                    secs,
                                );
                            }
                            Some(DeepAction::Off) => {
                                handle
                                    .state::<AppState>()
                                    .engine
                                    .set_active(false, ActivationReason::Manual, None);
                            }
                            Some(DeepAction::Toggle) => {
                                handle.state::<AppState>().engine.toggle();
                            }
                            None => tracing::warn!("unrecognized deep link: {url}"),
                        }
                    }
                });
            }

            // Floating HUD pill (optional, all platforms).
            {
                let hud = tauri::WebviewWindowBuilder::new(
                    app,
                    "hud",
                    tauri::WebviewUrl::default(),
                )
                .title("EspressoMacchiato HUD")
                .inner_size(210.0, 56.0)
                .decorations(false)
                .resizable(false)
                .visible(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .transparent(true)
                .build()?;
                if app
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .hud_enabled
                {
                    let _ = hud.show();
                }
            }

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

                    // Battery line in the tray menu (Linux parity for the
                    // popover's battery strip; harmless everywhere).
                    let battery_line = match (snapshot.percent, snapshot.watts) {
                        (Some(p), Some(w)) => {
                            format!("{}% · {w:.1} W", p.round() as i64)
                        }
                        (Some(p), None) => format!("{}%", p.round() as i64),
                        (None, Some(w)) => format!("AC · {w:.1} W"),
                        (None, None) => "—".to_string(),
                    };
                    tray::set_battery_line(&sampler_handle, &battery_line);

                    // Refresh the tray (ring, state) with live data, and
                    // feed the weekly stats.
                    let status = state.engine.status().await.ok();
                    if let Some(status) = &status {
                        tray::sync(&sampler_handle, status);
                    }
                    if let Some(hub) = sampler_handle.try_state::<StatsHub>() {
                        let now = chrono::Local::now();
                        let mut st = hub.stats.lock().unwrap();
                        st.rollover(now.date_naive());
                        if let Some(status) = &status {
                            if status.state != "off" {
                                st.week_active_secs += SAMPLE_EVERY.as_secs();
                            }
                            st.track_pokes(status.poke_count);
                        }
                        if let Some(health) = snapshot.health_percent {
                            st.track_health(&now.format("%Y-%m").to_string(), health);
                        }

                        let lang = backend_language(
                            &state.settings.lock().unwrap().language,
                        );
                        // Friday 17:00+ weekly report, once per week.
                        use chrono::{Datelike, Timelike};
                        if now.weekday() == chrono::Weekday::Fri
                            && now.hour() >= 17
                            && st.last_report_week != st.week_start
                        {
                            st.last_report_week = st.week_start.clone();
                            let (title, body) =
                                stats::report_message(&st, snapshot.health_percent, &lang);
                            tray::notify(&sampler_handle, &title, &body);
                        }
                        // Monthly calibration reminder (battery coach).
                        let month = now.format("%Y-%m").to_string();
                        let calibration_on =
                            state.settings.lock().unwrap().alerts.calibration_reminder;
                        if calibration_on
                            && snapshot.percent.is_some()
                            && st.last_calibration_month != month
                        {
                            st.last_calibration_month = month;
                            let (title, body) = stats::calibration_message(&lang);
                            tray::notify(&sampler_handle, &title, &body);
                        }
                        stats::save(&hub.path, &st);
                    }

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
                            let remaining =
                                status.as_ref().and_then(|s| s.remaining_secs);
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
            .inner_size(360.0, 460.0)
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
            commands::request_permission,
            commands::get_next_meeting_end,
            commands::get_stats,
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

#[cfg(test)]
mod tests {
    use super::{parse_deep_link, DeepAction};

    #[test]
    fn deep_links_parse() {
        assert_eq!(parse_deep_link("espresso://off"), Some(DeepAction::Off));
        assert_eq!(
            parse_deep_link("espresso://toggle/"),
            Some(DeepAction::Toggle)
        );
        assert_eq!(parse_deep_link("espresso://on"), Some(DeepAction::On(None)));
        assert_eq!(
            parse_deep_link("espresso://on?for=90m"),
            Some(DeepAction::On(Some(5400)))
        );
        assert_eq!(
            parse_deep_link("espresso://on?for=2h"),
            Some(DeepAction::On(Some(7200)))
        );
        assert_eq!(
            parse_deep_link("espresso://on?for=3600"),
            Some(DeepAction::On(Some(3600)))
        );
        assert_eq!(parse_deep_link("espresso://nope"), None);
        assert_eq!(parse_deep_link("https://example.com"), None);
    }
}
