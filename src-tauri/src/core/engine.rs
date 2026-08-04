//! The engine: an actor that owns the platform handles and drives the
//! inhibit + poke loop. Commands come in through an mpsc channel, events go
//! out through a callback (Tauri event emitter in production, a Vec in tests).

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio::time::MissedTickBehavior;

use super::events::{EngineEvent, PokeReport, StatusSnapshot};
use super::state::{ActivationReason, EngineState, StopReason, SuspendReason};
use crate::platform::{
    ActivityStrategy, Degradation, DegradationKind, InhibitOptions, Platform, PlatformError,
};

pub const MIN_INTERVAL_SECS: u64 = 10;
pub const MAX_INTERVAL_SECS: u64 = 240;
pub const DEFAULT_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct EngineSettings {
    pub interval: Duration,
    /// Skip a poke cycle when the user is already active
    /// (idle < interval / 2). Reduces injections to zero during real use.
    pub pause_when_input_recent: bool,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(DEFAULT_INTERVAL_SECS),
            pause_when_input_recent: true,
        }
    }
}

pub fn clamp_interval_secs(secs: u64) -> u64 {
    secs.clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS)
}

#[derive(Debug)]
pub enum Command {
    SetActive {
        on: bool,
        reason: ActivationReason,
    },
    Toggle,
    SetIntervalSecs(u64),
    SetPauseWhenInputRecent(bool),
    SetStrategy(ActivityStrategy),
    Suspend(SuspendReason),
    Resume,
    PokeNow {
        reply: oneshot::Sender<PokeReport>,
    },
    GetStatus {
        reply: oneshot::Sender<StatusSnapshot>,
    },
    GetIdle {
        reply: oneshot::Sender<Result<f64, String>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::UnboundedSender<Command>,
}

impl EngineHandle {
    fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn set_active(&self, on: bool, reason: ActivationReason) {
        self.send(Command::SetActive { on, reason });
    }

    pub fn toggle(&self) {
        self.send(Command::Toggle);
    }

    pub fn set_interval_secs(&self, secs: u64) {
        self.send(Command::SetIntervalSecs(secs));
    }

    pub fn set_pause_when_input_recent(&self, on: bool) {
        self.send(Command::SetPauseWhenInputRecent(on));
    }

    pub fn set_strategy(&self, s: ActivityStrategy) {
        self.send(Command::SetStrategy(s));
    }

    pub fn suspend(&self, reason: SuspendReason) {
        self.send(Command::Suspend(reason));
    }

    pub fn resume(&self) {
        self.send(Command::Resume);
    }

    pub fn shutdown(&self) {
        self.send(Command::Shutdown);
    }

    pub async fn poke_now(&self) -> Result<PokeReport, String> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::PokeNow { reply: tx });
        rx.await.map_err(|_| "engine stopped".to_string())
    }

    pub async fn status(&self) -> Result<StatusSnapshot, String> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::GetStatus { reply: tx });
        rx.await.map_err(|_| "engine stopped".to_string())
    }

    pub async fn idle_seconds(&self) -> Result<f64, String> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::GetIdle { reply: tx });
        rx.await.map_err(|_| "engine stopped".to_string())?
    }
}

pub type EventSink = Box<dyn Fn(EngineEvent) + Send + 'static>;

struct Engine {
    platform: Platform,
    settings: EngineSettings,
    state: EngineState,
    degradations: Vec<Degradation>,
    poke_count: u64,
    last_poke_unix_ms: Option<u64>,
    last_poke_ok: Option<bool>,
    last_poke_error: Option<String>,
    next_poke_at: Option<Instant>,
    on_event: EventSink,
}

/// Build the engine and return its handle plus the future that runs it.
/// The caller decides where to spawn it (`tauri::async_runtime::spawn` in
/// the app, `tokio::spawn` in tests).
pub fn start(
    platform: Platform,
    settings: EngineSettings,
    on_event: EventSink,
) -> (EngineHandle, impl std::future::Future<Output = ()> + Send) {
    let (tx, rx) = mpsc::unbounded_channel();
    let engine = Engine {
        platform,
        settings,
        state: EngineState::Off {
            reason: StopReason::NeverStarted,
        },
        degradations: Vec::new(),
        poke_count: 0,
        last_poke_unix_ms: None,
        last_poke_ok: None,
        last_poke_error: None,
        next_poke_at: None,
        on_event,
    };
    (EngineHandle { tx }, engine.run(rx))
}

impl Engine {
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Command>) {
        let mut ticker: Option<tokio::time::Interval> = None;
        let mut armed_interval: Option<Duration> = None;

        loop {
            tokio::select! {
                biased;
                maybe_cmd = rx.recv() => {
                    match maybe_cmd {
                        None | Some(Command::Shutdown) => break,
                        Some(cmd) => self.handle(cmd).await,
                    }
                }
                _ = async { ticker.as_mut().unwrap().tick().await }, if ticker.is_some() => {
                    self.on_tick();
                }
            }

            // Reconcile the ticker with the desired state. Recreating it on
            // every change (instead of mutating in place) keeps activation
            // idempotent and makes drift behavior explicit.
            let want = self.state.should_poke().then_some(self.settings.interval);
            if want != armed_interval {
                ticker = want.map(|iv| {
                    let mut t = tokio::time::interval_at(tokio::time::Instant::now() + iv, iv);
                    // After a forced-sleep resume, do NOT burst missed pokes.
                    t.set_missed_tick_behavior(MissedTickBehavior::Delay);
                    t
                });
                armed_interval = want;
                self.next_poke_at = want.map(|iv| Instant::now() + iv);
            }
        }

        // Guaranteed cleanup: whatever the state, release the inhibitor.
        self.deactivate(StopReason::Quit);
    }

    async fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::SetActive { on, reason } => {
                if on {
                    self.activate(reason);
                } else {
                    self.deactivate(StopReason::UserToggle);
                }
            }
            Command::Toggle => {
                if self.state.is_on() {
                    self.deactivate(StopReason::UserToggle);
                } else {
                    self.activate(ActivationReason::Manual);
                }
            }
            Command::SetIntervalSecs(secs) => {
                self.settings.interval = Duration::from_secs(clamp_interval_secs(secs));
                self.emit_state();
            }
            Command::SetPauseWhenInputRecent(on) => {
                self.settings.pause_when_input_recent = on;
                self.emit_state();
            }
            Command::SetStrategy(s) => {
                if let Err(e) = self.platform.simulator.set_strategy(s) {
                    tracing::warn!("set_strategy failed: {e}");
                }
                self.emit_state();
            }
            Command::Suspend(reason) => self.suspend(reason),
            Command::Resume => self.resume(),
            Command::PokeNow { reply } => {
                let report = self.manual_poke().await;
                let _ = reply.send(report);
            }
            Command::GetStatus { reply } => {
                let _ = reply.send(self.snapshot());
            }
            Command::GetIdle { reply } => {
                let _ = reply.send(self.platform.idle.idle_seconds().map_err(|e| e.to_string()));
            }
            Command::Shutdown => unreachable!("handled in run loop"),
        }
    }

    /// Idempotent: activating while already on must not stack assertions.
    fn activate(&mut self, reason: ActivationReason) {
        if self.state.is_on() {
            return;
        }
        self.enter_running(reason, None);
    }

    fn enter_running(&mut self, reason: ActivationReason, deadline: Option<Instant>) {
        let mut degradations = (self.platform.preflight)();
        if let Err(e) = self.platform.inhibitor.acquire(InhibitOptions::default()) {
            degradations.push(Degradation {
                what: DegradationKind::InhibitUnavailable,
                detail: e.to_string(),
                help: None,
            });
        }
        self.degradations = degradations;

        let poke_blocked = self
            .degradations
            .iter()
            .find(|d| d.what == DegradationKind::PokeUnavailable)
            .map(|d| d.detail.clone());

        self.state = match poke_blocked {
            Some(detail) => EngineState::Degraded {
                reason,
                deadline,
                detail,
            },
            None => EngineState::Active { reason, deadline },
        };
        self.emit_state();
    }

    fn deactivate(&mut self, reason: StopReason) {
        if !self.state.is_on() {
            return;
        }
        if let Err(e) = self.platform.inhibitor.release() {
            tracing::warn!("inhibitor release failed: {e}");
        }
        self.next_poke_at = None;
        self.state = EngineState::Off { reason };
        self.emit_state();
    }

    fn suspend(&mut self, reason: SuspendReason) {
        if !self.state.is_on() || matches!(self.state, EngineState::Suspended { .. }) {
            return;
        }
        let resume_as = self
            .state
            .activation_reason()
            .unwrap_or(ActivationReason::Manual);
        let deadline = self.state.deadline();
        if let Err(e) = self.platform.inhibitor.release() {
            tracing::warn!("inhibitor release failed: {e}");
        }
        self.next_poke_at = None;
        self.state = EngineState::Suspended {
            reason,
            resume_as,
            deadline,
        };
        self.emit_state();
    }

    fn resume(&mut self) {
        if let EngineState::Suspended {
            resume_as,
            deadline,
            ..
        } = self.state
        {
            self.enter_running(resume_as, deadline);
        }
    }

    fn on_tick(&mut self) {
        if let Some(deadline) = self.state.deadline() {
            if Instant::now() >= deadline {
                self.deactivate(StopReason::TimerExpired);
                return;
            }
        }
        if !self.state.should_poke() {
            return;
        }
        self.next_poke_at = Some(Instant::now() + self.settings.interval);
        let report = self.poke_cycle(false);
        let poke_count = self.poke_count;
        (self.on_event)(EngineEvent::Poke { report, poke_count });
    }

    /// One poke cycle. `manual` bypasses the input-recent skip.
    fn poke_cycle(&mut self, manual: bool) -> PokeReport {
        let idle_before = self.platform.idle.idle_seconds().ok();

        if !manual && self.settings.pause_when_input_recent {
            if let Some(idle) = idle_before {
                if idle < self.settings.interval.as_secs_f64() / 2.0 {
                    return PokeReport {
                        ok: false,
                        skipped: true,
                        error: None,
                        idle_before,
                        idle_after: None,
                    };
                }
            }
        }

        match self.platform.simulator.poke() {
            Ok(()) => {
                self.poke_count += 1;
                self.last_poke_ok = Some(true);
                self.last_poke_error = None;
                self.last_poke_unix_ms = unix_ms();
                // A working poke while Degraded means the permission came
                // back: promote to Active.
                if let EngineState::Degraded {
                    reason, deadline, ..
                } = self.state
                {
                    self.degradations
                        .retain(|d| d.what != DegradationKind::PokeUnavailable);
                    self.state = EngineState::Active { reason, deadline };
                    self.emit_state();
                }
                PokeReport {
                    ok: true,
                    skipped: false,
                    error: None,
                    idle_before,
                    idle_after: None,
                }
            }
            Err(e) => {
                let msg = e.to_string();
                self.last_poke_ok = Some(false);
                self.last_poke_error = Some(msg.clone());
                if let PlatformError::PermissionDenied(detail) = &e {
                    if let EngineState::Active { reason, deadline } = self.state {
                        self.degradations.push(Degradation {
                            what: DegradationKind::PokeUnavailable,
                            detail: detail.clone(),
                            help: None,
                        });
                        self.state = EngineState::Degraded {
                            reason,
                            deadline,
                            detail: detail.clone(),
                        };
                        self.emit_state();
                    }
                }
                PokeReport {
                    ok: false,
                    skipped: false,
                    error: Some(msg),
                    idle_before,
                    idle_after: None,
                }
            }
        }
    }

    /// "Try now" button: poke once regardless of state, then re-read idle so
    /// the user sees the before/after delta.
    async fn manual_poke(&mut self) -> PokeReport {
        let mut report = self.poke_cycle(true);
        // Give the OS a moment to process the injected event before
        // re-reading the idle counter.
        tokio::time::sleep(Duration::from_millis(200)).await;
        report.idle_after = self.platform.idle.idle_seconds().ok();
        let poke_count = self.poke_count;
        (self.on_event)(EngineEvent::Poke {
            report: report.clone(),
            poke_count,
        });
        report
    }

    fn snapshot(&self) -> StatusSnapshot {
        let now = Instant::now();
        StatusSnapshot {
            state: self.state.label(),
            state_detail: match &self.state {
                EngineState::Off { reason } => format!("{reason:?}"),
                EngineState::Active { reason, .. } => format!("{reason:?}"),
                EngineState::Suspended { reason, .. } => format!("{reason:?}"),
                EngineState::Degraded { detail, .. } => detail.clone(),
            },
            interval_secs: self.settings.interval.as_secs(),
            strategy: self.platform.simulator.strategy(),
            available_strategies: self.platform.simulator.available_strategies(),
            pause_when_input_recent: self.settings.pause_when_input_recent,
            inhibitor_active: self.platform.inhibitor.is_active(),
            poke_count: self.poke_count,
            last_poke_unix_ms: self.last_poke_unix_ms,
            last_poke_ok: self.last_poke_ok,
            last_poke_error: self.last_poke_error.clone(),
            next_poke_in_secs: self
                .next_poke_at
                .map(|t| t.saturating_duration_since(now).as_secs()),
            remaining_secs: self
                .state
                .deadline()
                .map(|d| d.saturating_duration_since(now).as_secs()),
            idle_source: self.platform.idle.source().to_string(),
            degradations: self.degradations.clone(),
        }
    }

    fn emit_state(&self) {
        (self.on_event)(EngineEvent::StateChanged {
            status: self.snapshot(),
        });
    }
}

fn unix_ms() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::platform::mock::{mock_platform, MockFailure, SharedMock};

    type Events = Arc<Mutex<Vec<EngineEvent>>>;

    fn start_engine() -> (EngineHandle, SharedMock, Events) {
        let (platform, shared) = mock_platform();
        let events: Events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let (handle, fut) = start(
            platform,
            EngineSettings::default(),
            Box::new(move |e| sink.lock().unwrap().push(e)),
        );
        tokio::spawn(fut);
        (handle, shared, events)
    }

    /// Let the actor drain its mailbox / run due timers.
    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    async fn advance(secs: u64) {
        tokio::time::advance(Duration::from_secs(secs)).await;
        settle().await;
    }

    #[tokio::test(start_paused = true)]
    async fn pokes_exactly_once_per_interval() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        for _ in 0..5 {
            advance(60).await;
        }
        assert_eq!(m.lock().unwrap().pokes, 5);

        // Just before the next tick: still 5.
        advance(59).await;
        assert_eq!(m.lock().unwrap().pokes, 5);
    }

    #[tokio::test(start_paused = true)]
    async fn no_burst_after_time_jump() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        advance(60).await;
        assert_eq!(m.lock().unwrap().pokes, 1);

        // Simulated forced-sleep resume: a 10-minute jump must yield exactly
        // one delayed poke, not ten.
        advance(600).await;
        assert_eq!(m.lock().unwrap().pokes, 2);

        advance(60).await;
        assert_eq!(m.lock().unwrap().pokes, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn activation_is_idempotent_and_release_happens_once() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.set_active(true, ActivationReason::Manual);
        h.set_active(true, ActivationReason::Hotkey);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(m.lock().unwrap().acquires, 1);

        h.set_active(false, ActivationReason::Manual);
        h.set_active(false, ActivationReason::Manual);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "off");
        assert_eq!(m.lock().unwrap().releases, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn suspended_releases_inhibitor_and_stops_poking() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        h.suspend(SuspendReason::OnBattery);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "suspended");
        assert_eq!(m.lock().unwrap().releases, 1);

        advance(600).await;
        assert_eq!(m.lock().unwrap().pokes, 0, "no pokes while suspended");

        h.resume();
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(m.lock().unwrap().acquires, 2);

        advance(60).await;
        assert_eq!(m.lock().unwrap().pokes, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn skips_poke_when_user_recently_active() {
        let (h, m, events) = start_engine();
        m.lock().unwrap().idle_secs = 5.0; // < interval / 2
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        advance(60).await;
        assert_eq!(m.lock().unwrap().pokes, 0);
        let skipped = events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, EngineEvent::Poke { report, .. } if report.skipped));
        assert!(skipped, "skip must be reported, not silent");

        m.lock().unwrap().idle_secs = 100.0;
        advance(60).await;
        assert_eq!(m.lock().unwrap().pokes, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn permission_denied_poke_degrades_then_recovers() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        m.lock().unwrap().poke_fail = Some(MockFailure::PermissionDenied);
        advance(60).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "degraded");
        assert!(status.inhibitor_active, "inhibitor stays while degraded");

        m.lock().unwrap().poke_fail = None;
        advance(60).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert!(status.degradations.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn transient_poke_error_keeps_active_state() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        m.lock().unwrap().poke_fail = Some(MockFailure::Other);
        advance(60).await;
        let status = h.status().await.unwrap();
        assert_eq!(
            status.state, "active",
            "non-permission errors don't degrade"
        );
        assert_eq!(status.last_poke_ok, Some(false));
        assert!(status.last_poke_error.is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn preflight_degradation_activates_as_degraded() {
        let (platform, shared) = mock_platform();
        shared.lock().unwrap().degradations.push(Degradation {
            what: DegradationKind::PokeUnavailable,
            detail: "missing permission".into(),
            help: Some("grant it".into()),
        });
        let (h, fut) = start(platform, EngineSettings::default(), Box::new(|_| {}));
        tokio::spawn(fut);

        h.set_active(true, ActivationReason::Manual);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "degraded");
        assert!(status.inhibitor_active);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_releases_inhibitor() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual);
        h.status().await.unwrap();

        h.shutdown();
        settle().await;
        assert_eq!(m.lock().unwrap().releases, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn interval_is_clamped() {
        let (h, _, _) = start_engine();
        h.set_interval_secs(1);
        assert_eq!(h.status().await.unwrap().interval_secs, MIN_INTERVAL_SECS);
        h.set_interval_secs(9999);
        assert_eq!(h.status().await.unwrap().interval_secs, MAX_INTERVAL_SECS);
        h.set_interval_secs(60);
        assert_eq!(h.status().await.unwrap().interval_secs, 60);
    }

    #[tokio::test(start_paused = true)]
    async fn manual_poke_works_even_when_off() {
        let (h, m, _) = start_engine();
        let report = h.poke_now().await.unwrap();
        assert!(report.ok);
        assert!(!report.skipped);
        assert_eq!(m.lock().unwrap().pokes, 1);
    }
}
