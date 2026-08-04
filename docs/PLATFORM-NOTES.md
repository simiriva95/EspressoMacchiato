# Platform notes

Every OS API this app touches, where it is documented, how it was verified,
and what does not work. Update this file with observed numbers whenever a
platform path changes.

## API plan (M1)

### macOS (target 12+)

| Trait | API | Source / verification |
|---|---|---|
| SleepInhibitor | `IOPMAssertionCreateWithName` / `IOPMAssertionRelease` (IOKit), assertion types `kIOPMAssertionTypePreventUserIdleSystemSleep` + `kIOPMAssertionTypePreventUserIdleDisplaySleep`, level `kIOPMAssertionLevelOn = 255` | Apple IOKit/IOPMLib.h documentation. No maintained Rust crate binds these — declared as `extern "C"` in `platform/macos/ffi.rs`. Verified at runtime with `pmset -g assertions` (see report below). |
| ActivitySimulator | `CGEventSourceCreate(kCGEventSourceStateHIDSystemState)`, `CGEventCreate` + `CGEventGetLocation` (current cursor), `CGEventCreateMouseEvent(kCGEventMouseMoved)`, `CGEventCreateKeyboardEvent` (F15 = keycode `0x71`), `CGEventPost(kCGHIDEventTap)` | Bound by `core-graphics 0.24` — signatures verified against crate source (`event.rs:533-610`, `event_source.rs:27`): `CGEventSource::new`, `CGEvent::new`, `CGEvent::new_mouse_event`, `CGEvent::new_keyboard_event`, `CGEvent::post`, `CGEvent::location`. |
| IdleReader | `CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateCombinedSessionState, kCGAnyInputEventType)` | Not bound by core-graphics 0.24 → `extern "C"` in `ffi.rs`. Constants verified against crate source: `CombinedSessionState = 0`; `kCGAnyInputEventType = 0xFFFFFFFF` (CGEventTypes.h: `(CGEventType)(~0)`). Same source Electron's `powerMonitor.getSystemIdleTime()` uses, which is why it is the right metric to show. |
| Permissions | `AXIsProcessTrusted()` (ApplicationServices/HIServices) for polling; deep link `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility` | Apple AXUIElement.h. `AXIsProcessTrustedWithOptions(prompt: true)` deferred to the M3 onboarding wizard ("Grant" button). |
| PowerMonitor (M4) | `IOServiceMatching("AppleSmartBattery")` + `IOServiceGetMatchingService` + `IORegistryEntryCreateCFProperties` (IOKitLib.h, extern in `ffi.rs`). Keys: `CycleCount`, `DesignCapacity`, `AppleRawMaxCapacity`, `Temperature` (¹⁄₁₀₀ °C), `Voltage` (mV), `Amperage` (mA, signed), `TimeRemaining` (min, 65535 = unknown), `ExternalConnected`, `IsCharging`, `CurrentCapacity` (percent on Apple Silicon, raw mAh on Intel — both handled). CPU/RAM/uptime/top processes via sysinfo. | **Verified on this hardware** (`cargo test -- --ignored platform::macos::power`): 11 %, 125 cycles, health 94.6 % (5913/6249 mAh), 30.7 °C, 10.798 V, −2009 mA discharging, 21.7 W, 21 min to empty — matches `pmset -g batt`. |
| Screen lock probe (M2) | `CGSessionCopyCurrentDictionary` → `CGSSessionScreenIsLocked` | CoreGraphics/CGSession.h; key present only while locked. |
| AC probe (M2, interim) | `pmset -g ps` parse | To be replaced by IOPSCopyPowerSourcesInfo FFI (docs/IDEAS.md). |

### Linux (target Ubuntu 22.04+, Fedora 39+, Arch; X11 and Wayland)

| Trait | API | Source / verification |
|---|---|---|
| SleepInhibitor | 1. `org.freedesktop.login1.Manager.Inhibit("idle:sleep", …, "block")` on the system bus → keeps the returned fd open, closes it on release. 2. `org.freedesktop.ScreenSaver.Inhibit/UnInhibit` (session bus) → cookie. 3. `org.gnome.SessionManager.Inhibit` with flag 8 (idle) when GNOME is detected. | freedesktop logind and idle-inhibition D-Bus specs; gnome-session source. Implemented with `zbus 5` blocking API. Graceful degradation: whatever succeeds is held; if nothing does → `Degraded` naming the detected DE. |
| ActivitySimulator | Default backend: uinput virtual pointer via `evdev 0.13` (`VirtualDevice::builder()`, `REL_X`/`REL_Y` axes + `BTN_LEFT` declared so compositors accept the device; poke = `REL_X 0, REL_Y 0` + automatic `SYN_REPORT`). Fallback backend (X11 only): XTest `FakeInput(MotionNotify, detail=1 relative, dx=0, dy=0)` via `x11rb 0.13` `xtest` feature. | kernel uinput docs; XTEST extension spec; crate sources. `/dev/uinput` needs the shipped udev rule (`packaging/linux/99-espressomacchiato-uinput.rules`) + `input` group membership. |
| IdleReader | Cascade, first that answers wins and is reported as `source`: X11 `XScreenSaverQueryInfo` (ms) → `org.gnome.Mutter.IdleMonitor.GetIdletime` (ms) → `org.freedesktop.ScreenSaver.GetSessionIdleTime` (s, KDE). None → `PlatformError::Unsupported`, UI shows "not detectable on this compositor". | XScreenSaver extension spec; Mutter and KDE D-Bus interfaces. |
| Session detection | `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, `DISPLAY`, `XDG_CURRENT_DESKTOP` read once, exposed in diagnostics. | freedesktop env conventions. |
| PowerMonitor (M4) | `/sys/class/power_supply/*`: `type`, `online`, `capacity`, `status`, `cycle_count`, `voltage_now` (µV), `current_now` (µA), `power_now` (µW), `temp` (¹⁄₁₀ °C), `charge_now/full/full_design` (µAh) **or** `energy_now/full/full_design` (µWh) — both families normalized; mAh is intentionally None with the energy family (a fake conversion would need a voltage assumption). | Kernel power_supply class ABI docs. Unit-tested against a fake sysfs tree (both families + no-battery desktop). |
| Screen lock probe (M2) | logind `Session.LockedHint` on `/org/freedesktop/login1/session/auto` | logind D-Bus API. |

## Known risks / spec pushback (M1)

1. **Zero-delta `MouseMoved` may be coalesced.** On some macOS versions /
   setups (mouse-utility software), a synthetic move to the same position can
   be filtered before it reaches the idle counter. This is exactly what the
   Idle Monitor's "Test now" button exists to verify; `HarmlessKeyTap` (F15)
   is the alternative. Empirical result below.
2. **F15 is not always inert.** Some external keyboards physically have
   F13–F15 historically mapped to brightness on old Apple displays. Injection
   is still harmless on modern systems, but it is a strategy, not the default.
3. **Zero-delta `REL_X/REL_Y` on Linux may be dropped by libinput** before it
   reaches the compositor's idle tracking (libinput discards pointer motion
   with no displacement in some versions). If verification on real hardware
   shows this, the uinput poke should switch to `KEY_F15` down/up, which is
   the same trick with no filtering risk. Not changed pre-emptively: needs a
   real X11/Wayland session to measure, and the spec's acceptance test (idle
   counter observed resetting) decides.
4. **XTest under XWayland** resets X server idle, not the compositor's —
   documented as not sufficient on Wayland-native setups.
5. **`CGEventPost` in dev vs bundle**: the Accessibility grant follows the
   *responsible process*. Under `tauri dev` that is usually the terminal app;
   for the bundled .app it is the app itself and it resets on re-signing.

## Verification report

### macOS — 2026-08-04, macOS 26 (Darwin 25.3.0), Apple Silicon, Xcode 26.6

Run: `cargo test --lib -- --ignored --nocapture` in `src-tauri/`.

| Check | Result |
|---|---|
| `IOPMAssertionCreateWithName` (both types) | ✅ `pmset -g assertions` shows `PreventUserIdleSystemSleep` and `PreventUserIdleDisplaySleep` named "EspressoMacchiato: keep the machine awake and present" while held; both gone after `release()`. |
| `IOPMAssertionRelease` on Drop | ✅ same test releases via `release()`; Drop path shares the code. Assertions also die with the process (verified by killing the smoke-run binary and re-checking `pmset -g assertions`). |
| `CGEventSourceSecondsSinceLastEventType(CombinedSessionState, kCGAnyInputEventType)` | ✅ returned `1.675 s` during an interactive session — plausible and monotonic. |
| `AXIsProcessTrusted` gate before `CGEventPost` | ✅ untrusted test process → `PlatformError::PermissionDenied("Accessibility permission not granted")`, exactly the Degraded path. No silent drop. |
| `ZeroMouseMove` resets the idle counter | ❔ **not yet verified**: requires granting Accessibility to the running binary (a user action in System Settings that this dev session cannot perform). Run `zero_mouse_move_resets_idle_counter` after granting Accessibility to your terminal, or use the app's "Test now" button after granting it to the bundled app. The test asserts `idle_after < 1.0 s` after ≥2.5 s of accumulated idle. |
| Cursor does not move / no stray characters | ❔ pending the same grant (part of the P0 acceptance run). |

Engine-level guarantees (poke cadence, no burst after a time jump, single
release, Suspended silence, Degraded transitions) are covered by 15 unit
tests against the mock with `tokio::time::pause`.

### macOS acceptance checklist still open (needs a human at the keyboard)

1. Grant Accessibility, run the ignored poke test, record before/after numbers here.
2. 15-minute unattended run with the app Active: idle must never exceed `interval + 5 s`, cursor still, no characters in a foreground editor, display awake.
3. Toggle off → idle resumes growing, `pmset -g assertions` clean.

### Linux

_No Linux hardware in this development session. Code compiles (cross-check)
and follows the documented D-Bus/uinput/XTest interfaces; the numbers table
must be produced on a real X11 session before claiming P0 on Linux. CI
builds on ubuntu-22.04._
