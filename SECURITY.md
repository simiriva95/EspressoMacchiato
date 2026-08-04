# Security

## Attack surface

EspressoMacchiato deliberately keeps its surface small, but two capabilities
deserve scrutiny:

- **Input injection.** On macOS the app holds the Accessibility permission
  and posts synthetic HID events; on Linux it owns a virtual uinput pointer.
  The app only ever emits zero-delta pointer motion, a ±1px nudge, or an
  F15 key down/up — never coordinates or keycodes derived from user data.
  It never *reads* input: no event taps, no keyloggers, no `/dev/input`
  reads.
- **/dev/uinput group access (Linux).** The shipped udev rule grants the
  `input` group write access to uinput. That is a real capability grant to
  every process in your session running as your user — the same trade-off
  every uinput-based tool makes. If that is unacceptable in your threat
  model, use the XTest fallback on X11 (no special permissions) or don't
  install the rule.

## Data flow

There is none. Settings live in a local JSON file. Metrics history lives in
RAM and dies with the process. The app opens zero network connections; it
has no telemetry, no crash reporting, no accounts, no update pings (an
optional updater may come later and will be off until you consent).

## Reporting a vulnerability

Open a GitHub security advisory (Security → Report a vulnerability) or a
private report to the maintainer. Please do not open public issues for
exploitable problems. You can expect an acknowledgement within a week.
