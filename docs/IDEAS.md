# Ideas / out of scope for now

- **libei / reis for Wayland input injection.** The long-term correct path on
  Wayland: emulated input through the desktop portal
  (`org.freedesktop.portal.RemoteDesktop` → libei), user-consented, no uinput
  permissions needed. Not implemented in v1: portal support is still uneven
  across compositors and the API adds a consent dialog to first run. Revisit
  when GNOME ≥ 45 / KDE ≥ 6 coverage is the baseline.
- **Flatpak packaging.** The sandbox complicates `/dev/uinput` access and
  session D-Bus visibility; would likely require the RemoteDesktop portal
  (see libei above) to work at all. Not a v1 problem.
- **XTest key-tap strategy.** Would need keysym→keycode mapping via the
  XKB tables; uinput is the primary backend, so not worth the code yet.
- **`caffeinate -dimsu` fallback on macOS** behind a feature flag, if a setup
  ever surfaces where IOPMAssertion is unavailable. No known case; the spec
  prefers no child processes.
