# Changelog

All notable changes to this project are documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning: [SemVer](https://semver.org).

## [0.3.0] - 2026-08-05

### Changed

- Reorganized the main window into a wide **dashboard** with a nav rail
  (Presence / Energy / Activity / Settings) — the whole app is visible, not
  hidden behind a settings button. Left-clicking the tray icon opens it
  directly; the small popover is gone.
- Signature **brewing cup**: an animated cup whose coffee level is the live
  idle counter (fills toward the interval, drains on a poke) with bold
  rising steam, in both the dashboard hero and the menu-bar icon.
- The tray icon now animates (steam + coffee fill) and is drawn at runtime,
  tinted by state with a live progress ring.
- Refined toggles, display typeface (Bricolage Grotesque), staggered
  section entrance, reduced-motion aware throughout.

## [0.2.0] - 2026-08-05

### Added

- **Auto-activate while you're in a call** — CoreAudio input-running probe
  on macOS (read-only, no mic permission), PulseAudio/PipeWire on Linux.
- **Until end of meeting** duration — reads your calendar (macOS EventKit),
  permission requested lazily and hidden when denied.
- **Espresso shots** — 25-minute focus timers with a daily counter chip.
- **Weekly report** — protected-presence time, pokes, shots and a monthly
  battery-health trend, as a card and a Friday notification. Aggregates
  only, in a local stats.json; raw samples never persisted.
- **Battery coach** — optional monthly calibration reminder.
- **Floating HUD pill** — frameless always-on-top counter, draggable,
  double-click to toggle.
- **Live tray ring** — progress arc around the cup for timer or battery.
- **Accent themes** — colour swatches with contrast-safe on-accent text.
- **Steam sound** on activation (synthesized at runtime, off by default).
- **Deep links** `espresso://on|off|toggle` for Apple Shortcuts, scripts
  and the bundled Raycast script commands.
- Interactive energy charts (hover for value + time, 2h/4h/8h window),
  CPU and battery-temperature history.
- Stable self-signed macOS identity so the Accessibility grant survives
  updates; system Accessibility prompt on first launch.

### Changed

- Redesigned to a modern liquid-glass system (graphite base, translucent
  cards, caramel accent); the keep-awake control is a status chip + switch
  instead of a shouting headline; proportionate counters.
- True menu bar app on macOS (Accessory policy, no Dock icon).

## [0.1.0] - 2026-08-04

### Added

- Keep-awake core: sleep + display-sleep inhibition (IOPMAssertion on
  macOS; logind fd + ScreenSaver cookie + GNOME SessionManager on Linux)
  combined with invisible idle-counter resets (zero-delta CGEvent mouse
  move on macOS; uinput virtual pointer with XTest fallback on Linux).
- Idle Monitor with live OS idle counter and a "Test now" before/after
  check.
- Durations, weekday schedule windows (DST-safe), opt-in gates and
  input-recent poke skip.
- Global hotkey, start at login, single instance.
- Energy panel with battery health, cycles, temperature, watts, in-memory
  sparklines and top processes.
- Opt-in rate-limited alerts.
- Bilingual UI (EN/IT), light/dark, WCAG 2.2 AA, degraded mode with a live
  permission wizard.
- Atomic settings with schema versioning. No telemetry, no network calls.
