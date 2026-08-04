# Changelog

All notable changes to this project are documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning: [SemVer](https://semver.org).

## [0.1.0] - 2026-08-04

### Added

- Keep-awake core: sleep + display-sleep inhibition (IOPMAssertion on
  macOS; logind fd + ScreenSaver cookie + GNOME SessionManager on Linux)
  combined with invisible idle-counter resets (zero-delta CGEvent mouse
  move on macOS; uinput virtual pointer with XTest fallback on Linux).
- Idle Monitor: live OS idle counter with source, poke stats and a
  "Test now" before/after check.
- Durations (15m–4h, until a time, until end of day), weekday schedule
  windows (midnight-crossing, DST-safe), opt-in gates (on-battery with
  threshold, required process, screen lock) and input-recent poke skip.
- Global hotkey (default Cmd/Ctrl+Alt+E), start at login, activate on
  start, single instance.
- Energy panel: battery %, health, cycles, temperature, voltage, watts,
  time estimates, 2-hour in-memory sparklines, CPU/RAM/uptime and top
  processes. Unavailable metrics say so instead of showing zeros.
- Opt-in alerts with per-episode edge triggering and a 30-minute rate
  limit: unplug reminder, low battery (names the heaviest process),
  overheat, timer expired.
- Menu bar app on macOS (Accessory policy) with a vibrancy popover and
  optional compact menu bar text (countdown / battery / watts, max two).
- Bilingual UI (EN/IT), light/dark themes, WCAG 2.2 AA (axe-verified),
  liquid-glass design system.
- Degraded mode as a first-class state with a live permission wizard.
- Settings persisted atomically with schema versioning and corrupt-file
  quarantine. No telemetry, no network calls.
