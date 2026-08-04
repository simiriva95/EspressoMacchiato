# Architecture

## Layers

```
┌────────────────────── React UI (webview) ───────────────────────┐
│  windows/Settings · windows/Popover (macOS)                      │
│  features: engine · idle-monitor · power · onboarding            │
│  lib/ipc.ts — the only place that touches Tauri APIs             │
└───────────────▲──────────────────────────────▲───────────────────┘
        invoke() commands              events: engine://event,
        (commands.rs)                  power://sample
┌───────────────┴──────────────────────────────┴───────────────────┐
│ lib.rs (bootstrap): tray, plugins, settings, sampler task        │
│                                                                   │
│  core/engine.rs — actor owning the platform handles              │
│    state machine: Off / Active / Suspended / Degraded            │
│    poke ticker (MissedTickBehavior::Delay, no post-sleep burst)  │
│    gate tick (10 s): deadline, schedule windows, condition gates │
│  core/schedule.rs · core/conditions.rs — pure, unit-tested       │
│  alerts.rs — edge-triggered, rate-limited rules (pure)           │
│  config/ — settings.json, atomic writes, migrations              │
└───────────────▲───────────────────────────────────────────────────┘
                │ 5 traits + preflight probe (platform/mod.rs)
┌───────────────┴───────────────────────────────────────────────────┐
│ SleepInhibitor · ActivitySimulator · IdleReader · ConditionProbe │
│ PowerMonitor                                                      │
│  macos/: IOPMAssertion, CGEvent, CGEventSource, AppleSmartBattery│
│  linux/: logind/ScreenSaver D-Bus, uinput/XTest, XScreenSaver/   │
│          Mutter/KDE idle, /sys/class/power_supply                │
│  mock.rs: call-counting, fault-injecting — drives all core tests │
└───────────────────────────────────────────────────────────────────┘
```

## Engine state machine

```
        Off ──(toggle / hotkey / schedule opens / autostart)──▶ Active
  Active ──(gate unsatisfied: battery/process/lock)──▶ Suspended
  Suspended ──(gate satisfied)──▶ Active        (inhibitor re-acquired)
  Active ──(poke PermissionDenied)──▶ Degraded  (inhibitor kept, pokes retried)
  Degraded ──(poke succeeds)──▶ Active
  any ──(toggle off / timer / schedule closes / quit)──▶ Off (full release)
```

Invariants (unit-tested against the mock with a paused tokio clock):
activation idempotent (never stacks assertions), release exactly once,
no pokes while Suspended, no poke burst after a time jump, Degraded is
visible and recoverable.

## IPC

Commands are thin wrappers over the engine actor's mailbox
(`EngineHandle`). Events flow one way: the engine emits through a callback
that the bootstrap fans out to the tray, notifications and the webview.
The frontend `ipc.ts` module is the single seam — tests mock it, and a
browser demo implementation activates automatically outside Tauri so the
UI can be developed and screenshotted without the shell.

## Design decisions worth knowing

- **Wall-clock polling over absolute timers** for schedule/deadline: DST
  changes and forced sleep can't skip or double-fire anything (10 s gate
  tick bounds the latency).
- **Preflight as data**: platform capability problems become
  `Degradation { what, detail, help }` values that flow to the tray badge,
  a one-shot notification and the onboarding checklist — same source.
- **Everything Option**: power metrics that the hardware doesn't expose
  reach the UI as "not available", never fabricated zeros.
