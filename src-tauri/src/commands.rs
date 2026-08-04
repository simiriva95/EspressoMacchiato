//! #[tauri::command] endpoints exposed to the frontend.

use tauri::State;

use crate::core::engine::clamp_interval_secs;
use crate::core::events::{PokeReport, StatusSnapshot};
use crate::core::state::ActivationReason;
use crate::platform::{ActivityStrategy, Degradation};
use crate::AppState;

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusSnapshot, String> {
    state.engine.status().await
}

#[tauri::command]
pub async fn set_active(state: State<'_, AppState>, on: bool) -> Result<(), String> {
    state.engine.set_active(on, ActivationReason::Manual);
    Ok(())
}

#[tauri::command]
pub async fn toggle(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.toggle();
    Ok(())
}

#[tauri::command]
pub async fn set_interval_secs(state: State<'_, AppState>, secs: u64) -> Result<u64, String> {
    state.engine.set_interval_secs(secs);
    Ok(clamp_interval_secs(secs))
}

#[tauri::command]
pub async fn set_pause_when_input_recent(
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), String> {
    state.engine.set_pause_when_input_recent(on);
    Ok(())
}

#[tauri::command]
pub async fn set_strategy(
    state: State<'_, AppState>,
    strategy: ActivityStrategy,
) -> Result<(), String> {
    state.engine.set_strategy(strategy);
    Ok(())
}

#[tauri::command]
pub async fn poke_now(state: State<'_, AppState>) -> Result<PokeReport, String> {
    state.engine.poke_now().await
}

#[tauri::command]
pub async fn get_idle_seconds(state: State<'_, AppState>) -> Result<f64, String> {
    state.engine.idle_seconds().await
}

/// Live permission checklist for the onboarding / settings UI.
#[tauri::command]
pub fn get_permission_status(state: State<'_, AppState>) -> Vec<Degradation> {
    (state.preflight)()
}

/// Deep link to the OS panel where the missing permission is granted.
#[tauri::command]
pub fn open_permission_settings(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                None::<String>,
            )
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("no settings panel to open on this platform".into())
    }
}
