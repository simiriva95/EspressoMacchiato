# Contributing

## Setup

Prereqs: Rust stable, Node 22+, and on Linux the Tauri build deps:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev build-essential curl wget file libssl-dev
```

```bash
npm install
npm run tauri dev
```

## Checks (all must pass, CI enforces them)

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
npx tsc --noEmit && npm run lint && npm test
```

## Hardware tests

Platform code that touches real OS services is tested with `#[ignore]`d
tests, run locally only:

```bash
cd src-tauri && cargo test -- --ignored --nocapture
```

- macOS: `inhibitor_creates_and_releases_assertions` (verify with
  `pmset -g assertions`), `idle_reader_returns_plausible_value`,
  `zero_mouse_move_resets_idle_counter` (needs Accessibility granted to
  your terminal), `reads_a_plausible_battery_snapshot`.
- After running them, record the observed numbers in
  `docs/PLATFORM-NOTES.md` — that file is the source of truth for the
  compatibility table in the README.

The most valuable contribution right now: run the acceptance checklist on
Linux (X11, Wayland GNOME, Wayland KDE) and report numbers.

## Conventions

- Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`).
- Code, comments and commits in English; UI strings only via i18n (a CI
  test rejects hardcoded JSX strings).
- Platform-specific code lives behind the traits in `src-tauri/src/platform/`
  with a mock implementation — never `#[cfg]` in domain logic.
- Out-of-scope ideas go to `docs/IDEAS.md` instead of growing PRs.

## macOS signing (local builds)

macOS ties the Accessibility grant to the code signature. Ad-hoc signing
(`-`) produces a new identity on every rebuild, so the permission is lost
after each update. Create a free, local self-signed identity once:

```bash
./scripts/create-signing-identity.sh
```

Then build with it (the designated requirement becomes
`identifier + certificate root`, stable across rebuilds):

```bash
export APPLE_SIGNING_IDENTITY="EspressoMacchiato Local Signing"
npm run tauri build
```

If a stale grant lingers from an earlier ad-hoc build:
`tccutil reset Accessibility app.espressomacchiato`.

CI has no such keychain, so CI builds fall back to ad-hoc signing — fine
for distribution, where every user grants the permission once anyway.

## Releases

Tag `v X.Y.Z` → `release.yml` builds dmg (ad-hoc signed), deb/rpm/AppImage
and appends SHA256SUMS. Homebrew cask and AUR PKGBUILD are generated from
the templates in `packaging/` (fill version + sha256 from SHA256SUMS.txt);
AUR publication is manual.
