//! The engine: an actor that owns the platform handles and drives the
//! inhibit + poke loop. Commands come in through an mpsc channel, events go
//! out through a callback (Tauri event emitter in production, a Vec in tests).

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::NaiveDateTime;
use tokio::sync::{mpsc, oneshot};
// tokio Instant so deadlines/countdowns respect the paused clock in tests.
use tokio::time::{Instant, MissedTickBehavior};

use super::conditions::{required_suspension, GateFacts};
use super::events::{EngineEvent, PokeReport, StatusSnapshot};
use super::schedule;
use super::state::{ActivationReason, EngineState, StopReason, SuspendReason};
use crate::config::{ConditionsConfig, ScheduleConfig};
use crate::platform::{
    ActivityStrategy, Degradation, DegradationKind, InhibitOptions, Platform, PlatformError,
};

pub const MIN_INTERVAL_SECS: u64 = 10;
pub const MAX_INTERVAL_SECS: u64 = 240;
pub const DEFAULT_INTERVAL_SECS: u64 = 60;

/// Cadence of schedule/gate/deadline evaluation. Bounds how late a timer
/// or schedule transition can fire.
const GATE_TICK: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct EngineSettings {
    pub interval: Duration,
    pub schedule: ScheduleConfig,
    pub conditions: ConditionsConfig,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(DEFAULT_INTERVAL_SECS),
            schedule: ScheduleConfig::default(),
            conditions: ConditionsConfig::default(),
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
        /// None = indefinite. "Until HH:MM" is converted to seconds by the
        /// caller (schedule::seconds_until).
        duration_secs: Option<u64>,
    },
    Toggle,
    SetIntervalSecs(u64),
    SetStrategy(ActivityStrategy),
    /// Full settings push (persisted config → engine).
    ApplyConfig {
        interval_secs: u64,
        schedule: ScheduleConfig,
        conditions: ConditionsConfig,
    },
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

    pub fn set_active(&self, on: bool, reason: ActivationReason, duration_secs: Option<u64>) {
        self.send(Command::SetActive {
            on,
            reason,
            duration_secs,
        });
    }

    pub fn toggle(&self) {
        self.send(Command::Toggle);
    }

    pub fn set_interval_secs(&self, secs: u64) {
        self.send(Command::SetIntervalSecs(secs));
    }

    pub fn set_strategy(&self, s: ActivityStrategy) {
        self.send(Command::SetStrategy(s));
    }

    pub fn apply_config(
        &self,
        interval_secs: u64,
        schedule: ScheduleConfig,
        conditions: ConditionsConfig,
    ) {
        self.send(Command::ApplyConfig {
            interval_secs,
            schedule,
            conditions,
        });
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
pub type LocalClock = Arc<dyn Fn() -> NaiveDateTime + Send + Sync>;

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
    /// Total seconds of the current timed activation (for progress rings).
    active_total_secs: Option<u64>,
    /// Edge detector for the call gate (mic in use), same manual-off
    /// semantics as the schedule edge detector below.
    last_call_active: bool,
    /// Edge detector for schedule windows: acting on transitions only means
    /// a manual "off" during an open window is honored until it reopens.
    last_schedule_open: bool,
    now_local: LocalClock,
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
    start_with_clock(
        platform,
        settings,
        on_event,
        Arc::new(|| chrono::Local::now().naive_local()),
    )
}

/// Test entry point: inject the wall clock the schedule evaluates against.
pub fn start_with_clock(
    platform: Platform,
    settings: EngineSettings,
    on_event: EventSink,
    now_local: LocalClock,
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
        active_total_secs: None,
        last_call_active: false,
        last_schedule_open: false,
        now_local,
        on_event,
    };
    (EngineHandle { tx }, engine.run(rx))
}

impl Engine {
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Command>) {
        let mut ticker: Option<tokio::time::Interval> = None;
        let mut armed_interval: Option<Duration> = None;
        let mut gate_tick = tokio::time::interval(GATE_TICK);
        gate_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                biased;
                maybe_cmd = rx.recv() => {
                    match maybe_cmd {
                        None | Some(Command::Shutdown) => break,
                        Some(cmd) => self.handle(cmd).await,
                    }
                }
                _ = gate_tick.tick() => {
                    self.evaluate_gates();
                }
                _ = async { ticker.as_mut().unwrap().tick().await }, if ticker.is_some() => {
                    self.on_tick();
                }
            }

            // Reconcile the poke ticker with the desired state. Recreating
            // it on every change (instead of mutating in place) keeps
            // activation idempotent and makes drift behavior explicit.
            let want = self.state.should_poke().then_some(self.settings.interval);
            if want != armed_interval {
                ticker = want.map(|iv| {
                    let mut t = tokio::time::interval_at(Instant::now() + iv, iv);
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
            Command::SetActive {
                on,
                reason,
                duration_secs,
            } => {
                if on {
                    let deadline =
                        duration_secs.map(|secs| Instant::now() + Duration::from_secs(secs));
                    self.active_total_secs = duration_secs;
                    self.activate(reason, deadline);
                } else {
                    self.deactivate(StopReason::UserToggle);
                }
            }
            Command::Toggle => {
                if self.state.is_on() {
                    self.deactivate(StopReason::UserToggle);
                } else {
                    self.activate(ActivationReason::Manual, None);
                }
            }
            Command::SetIntervalSecs(secs) => {
                self.settings.interval = Duration::from_secs(clamp_interval_secs(secs));
                self.emit_state();
            }
            Command::SetStrategy(s) => {
                if let Err(e) = self.platform.simulator.set_strategy(s) {
                    tracing::warn!("set_strategy failed: {e}");
                }
                self.emit_state();
            }
            Command::ApplyConfig {
                interval_secs,
                schedule,
                conditions,
            } => {
                self.settings.interval = Duration::from_secs(clamp_interval_secs(interval_secs));
                self.settings.schedule = schedule;
                self.settings.conditions = conditions;
                // Re-arm the edge detector so an already-open window is
                // picked up (or a removed schedule stops mattering).
                self.last_schedule_open = false;
                self.evaluate_gates();
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
    fn activate(&mut self, reason: ActivationReason, deadline: Option<Instant>) {
        if self.state.is_on() {
            return;
        }
        self.enter_running(reason, deadline);
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
        self.active_total_secs = None;
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

    /// Periodic evaluation of deadline, schedule windows and gates. Polling
    /// on the wall clock (instead of precomputed absolute timers) is what
    /// makes DST changes and forced-sleep resumes a non-event.
    fn evaluate_gates(&mut self) {
        if let Some(deadline) = self.state.deadline() {
            if Instant::now() >= deadline {
                self.deactivate(StopReason::TimerExpired);
                return;
            }
        }

        if self.settings.conditions.auto_activate_on_call {
            if let Some(mic) = self.platform.conditions.mic_in_use() {
                if mic && !self.last_call_active && !self.state.is_on() {
                    self.enter_running(ActivationReason::Call, None);
                } else if !mic
                    && self.last_call_active
                    && self.state.is_on()
                    && self.state.activation_reason() == Some(ActivationReason::Call)
                {
                    self.deactivate(StopReason::CallEnded);
                }
                self.last_call_active = mic;
            }
        }

        if self.settings.schedule.enabled {
            let open = schedule::is_open(&self.settings.schedule.windows, (self.now_local)());
            if open && !self.last_schedule_open && !self.state.is_on() {
                self.enter_running(ActivationReason::Schedule, None);
            } else if !open
                && self.last_schedule_open
                && self.state.is_on()
                && self.state.activation_reason() == Some(ActivationReason::Schedule)
            {
                self.deactivate(StopReason::ScheduleClosed);
            }
            self.last_schedule_open = open;
        }

        if self.state.is_on() {
            let facts = self.collect_facts();
            let wanted = required_suspension(&self.settings.conditions, &facts);
            let suspended = matches!(self.state, EngineState::Suspended { .. });
            match (suspended, wanted) {
                (false, Some(reason)) => self.suspend(reason),
                (true, None) => self.resume(),
                _ => {}
            }
        }
    }

    /// Only probe what the enabled gates actually need: probes can be
    /// comparatively expensive (process scan, D-Bus round trip).
    fn collect_facts(&self) -> GateFacts {
        let cfg = &self.settings.conditions;
        let probe = &self.platform.conditions;
        GateFacts {
            on_ac: cfg.only_on_ac.then(|| probe.on_ac()).flatten(),
            battery_percent: cfg.only_on_ac.then(|| probe.battery_percent()).flatten(),
            screen_locked: cfg
                .pause_when_screen_locked
                .then(|| probe.screen_locked())
                .flatten(),
            required_process_running: cfg
                .only_when_process_running
                .then(|| probe.any_process_running(&cfg.process_names)),
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

        if !manual && self.settings.conditions.pause_when_input_recent {
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
            pause_when_input_recent: self.settings.conditions.pause_when_input_recent,
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
            duration_total_secs: self.active_total_secs,
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

    use chrono::{NaiveDate, NaiveDateTime};

    use super::*;
    use crate::config::ScheduleWindow;
    use crate::platform::mock::{mock_platform, MockFailure, SharedMock};

    type Events = Arc<Mutex<Vec<EngineEvent>>>;

    fn start_engine() -> (EngineHandle, SharedMock, Events) {
        start_engine_with(EngineSettings::default())
    }

    fn start_engine_with(settings: EngineSettings) -> (EngineHandle, SharedMock, Events) {
        let (h, m, e, _) = start_engine_full(settings, at(2026, 8, 4, 12, 0));
        (h, m, e)
    }

    type Clock = Arc<Mutex<NaiveDateTime>>;

    fn start_engine_full(
        settings: EngineSettings,
        initial_now: NaiveDateTime,
    ) -> (EngineHandle, SharedMock, Events, Clock) {
        let (platform, shared) = mock_platform();
        let events: Events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let clock: Clock = Arc::new(Mutex::new(initial_now));
        let clock_for_engine = clock.clone();
        let (handle, fut) = start_with_clock(
            platform,
            settings,
            Box::new(move |e| sink.lock().unwrap().push(e)),
            Arc::new(move || *clock_for_engine.lock().unwrap()),
        );
        tokio::spawn(fut);
        (handle, shared, events, clock)
    }

    fn at(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(hh, mm, 0)
            .unwrap()
    }

    fn activate(h: &EngineHandle) {
        h.set_active(true, ActivationReason::Manual, None);
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
        activate(&h);
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
        activate(&h);
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
        activate(&h);
        activate(&h);
        h.set_active(true, ActivationReason::Hotkey, None);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(m.lock().unwrap().acquires, 1);

        h.set_active(false, ActivationReason::Manual, None);
        h.set_active(false, ActivationReason::Manual, None);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "off");
        assert_eq!(m.lock().unwrap().releases, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn suspended_releases_inhibitor_and_stops_poking() {
        let (h, m, _) = start_engine();
        activate(&h);
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
        activate(&h);
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
        activate(&h);
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
        activate(&h);
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

        activate(&h);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "degraded");
        assert!(status.inhibitor_active);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_releases_inhibitor() {
        let (h, m, _) = start_engine();
        activate(&h);
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

    // ---- M2: durations, schedule, gates ----

    #[tokio::test(start_paused = true)]
    async fn timed_activation_expires() {
        let (h, m, _) = start_engine();
        h.set_active(true, ActivationReason::Manual, Some(15 * 60));
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        let remaining = status.remaining_secs.unwrap();
        assert!((890..=900).contains(&remaining), "remaining={remaining}");

        advance(14 * 60).await;
        assert_eq!(h.status().await.unwrap().state, "active");

        // Past the deadline (+ gate tick slack).
        advance(70).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "off");
        assert_eq!(status.state_detail, "TimerExpired");
        assert_eq!(m.lock().unwrap().releases, 1);
    }

    fn business_hours() -> EngineSettings {
        EngineSettings {
            schedule: ScheduleConfig {
                enabled: true,
                windows: vec![ScheduleWindow {
                    days: vec![0, 1, 2, 3, 4],
                    start: "09:00".into(),
                    end: "18:00".into(),
                }],
            },
            ..Default::default()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn schedule_opens_and_closes() {
        // Tuesday 08:55, before the window.
        let (h, m, _, clock) = start_engine_full(business_hours(), at(2026, 8, 4, 8, 55));
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "off");

        *clock.lock().unwrap() = at(2026, 8, 4, 9, 1);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(status.state_detail, "Schedule");

        *clock.lock().unwrap() = at(2026, 8, 4, 18, 1);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "off");
        assert_eq!(status.state_detail, "ScheduleClosed");
        assert_eq!(m.lock().unwrap().releases, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn manual_off_during_window_is_honored_until_reopen() {
        let (h, _, _, clock) = start_engine_full(business_hours(), at(2026, 8, 4, 10, 0));
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");

        // User turns it off mid-window: must stay off.
        h.set_active(false, ActivationReason::Manual, None);
        advance(120).await;
        assert_eq!(h.status().await.unwrap().state, "off");

        // Window closes overnight and reopens the next morning → active again.
        *clock.lock().unwrap() = at(2026, 8, 4, 19, 0);
        advance(10).await;
        *clock.lock().unwrap() = at(2026, 8, 5, 9, 5);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");
    }

    #[tokio::test(start_paused = true)]
    async fn manual_activation_outside_window_not_closed_by_schedule() {
        let (h, _, _, clock) = start_engine_full(business_hours(), at(2026, 8, 4, 20, 0));
        activate(&h);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");

        // Window opens then closes; a Manual activation must survive.
        *clock.lock().unwrap() = at(2026, 8, 5, 10, 0);
        advance(10).await;
        *clock.lock().unwrap() = at(2026, 8, 5, 19, 0);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(status.state_detail, "Manual");
    }

    #[tokio::test(start_paused = true)]
    async fn battery_gate_suspends_and_resumes() {
        let mut settings = EngineSettings::default();
        settings.conditions.only_on_ac = true;
        let (h, m, _) = start_engine_with(settings);
        activate(&h);
        h.status().await.unwrap();

        m.lock().unwrap().on_ac = Some(false);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "suspended");
        assert_eq!(status.state_detail, "OnBattery");
        assert_eq!(m.lock().unwrap().releases, 1);

        advance(600).await;
        assert_eq!(m.lock().unwrap().pokes, 0, "no pokes on battery");

        m.lock().unwrap().on_ac = Some(true);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");
        assert_eq!(m.lock().unwrap().acquires, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn process_gate_suspends_until_process_appears() {
        let mut settings = EngineSettings::default();
        settings.conditions.only_when_process_running = true;
        settings.conditions.process_names = vec!["teams".into()];
        let (h, m, _) = start_engine_with(settings);
        m.lock().unwrap().process_running = false;
        activate(&h);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "suspended");
        assert_eq!(status.state_detail, "ProcessNotRunning");

        m.lock().unwrap().process_running = true;
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");
    }

    #[tokio::test(start_paused = true)]
    async fn lock_gate_suspends_while_locked() {
        let mut settings = EngineSettings::default();
        settings.conditions.pause_when_screen_locked = true;
        let (h, m, _) = start_engine_with(settings);
        activate(&h);
        h.status().await.unwrap();

        m.lock().unwrap().screen_locked = Some(true);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "suspended");

        m.lock().unwrap().screen_locked = Some(false);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");
    }

    #[tokio::test(start_paused = true)]
    async fn apply_config_picks_up_already_open_window() {
        let (h, _, _, _clock) = start_engine_full(EngineSettings::default(), at(2026, 8, 4, 10, 0));
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "off");

        let s = business_hours();
        h.apply_config(60, s.schedule, s.conditions);
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(status.state_detail, "Schedule");
    }

    #[tokio::test(start_paused = true)]
    async fn call_gate_activates_and_releases_with_the_mic() {
        let mut settings = EngineSettings::default();
        settings.conditions.auto_activate_on_call = true;
        let (h, m, _) = start_engine_with(settings);
        m.lock().unwrap().mic_in_use = Some(false);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "off");

        m.lock().unwrap().mic_in_use = Some(true);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active");
        assert_eq!(status.state_detail, "Call");

        m.lock().unwrap().mic_in_use = Some(false);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "off");
        assert_eq!(status.state_detail, "CallEnded");
    }

    #[tokio::test(start_paused = true)]
    async fn manual_off_during_call_is_honored() {
        let mut settings = EngineSettings::default();
        settings.conditions.auto_activate_on_call = true;
        let (h, m, _) = start_engine_with(settings);
        m.lock().unwrap().mic_in_use = Some(true);
        advance(10).await;
        assert_eq!(h.status().await.unwrap().state, "active");

        h.set_active(false, ActivationReason::Manual, None);
        advance(60).await;
        assert_eq!(
            h.status().await.unwrap().state,
            "off",
            "same call, no reactivation after a manual off"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn manual_activation_survives_call_end() {
        let mut settings = EngineSettings::default();
        settings.conditions.auto_activate_on_call = true;
        let (h, m, _) = start_engine_with(settings);
        m.lock().unwrap().mic_in_use = Some(true);
        h.set_active(true, ActivationReason::Manual, None);
        advance(10).await;

        m.lock().unwrap().mic_in_use = Some(false);
        advance(10).await;
        let status = h.status().await.unwrap();
        assert_eq!(status.state, "active", "manual runs outlive the call");
        assert_eq!(status.state_detail, "Manual");
    }
}
