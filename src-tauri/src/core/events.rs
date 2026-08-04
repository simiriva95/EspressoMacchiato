//! Serializable snapshots and events pushed to the frontend and the tray.

use crate::platform::{ActivityStrategy, Degradation};

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusSnapshot {
    /// "off" | "active" | "suspended" | "degraded"
    pub state: &'static str,
    /// Human-oriented detail (stop reason, suspend reason, degradation).
    pub state_detail: String,
    pub interval_secs: u64,
    pub strategy: ActivityStrategy,
    pub available_strategies: Vec<ActivityStrategy>,
    pub pause_when_input_recent: bool,
    pub inhibitor_active: bool,
    pub poke_count: u64,
    pub last_poke_unix_ms: Option<u64>,
    pub last_poke_ok: Option<bool>,
    pub last_poke_error: Option<String>,
    pub next_poke_in_secs: Option<u64>,
    pub remaining_secs: Option<u64>,
    pub idle_source: String,
    pub degradations: Vec<Degradation>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PokeReport {
    pub ok: bool,
    pub skipped: bool,
    pub error: Option<String>,
    pub idle_before: Option<f64>,
    pub idle_after: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    StateChanged { status: StatusSnapshot },
    Poke { report: PokeReport, poke_count: u64 },
}
