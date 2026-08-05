# Homebrew cask, served straight from this repo as a tap (snapkit-style):
#   brew tap simiriva95/espressomacchiato https://github.com/simiriva95/EspressoMacchiato
#   brew install --cask --no-quarantine espresso-macchiato
#
# --no-quarantine is required until the app is notarized: Homebrew
# quarantines cask downloads by default and Gatekeeper rejects unnotarized
# quarantined apps. Bump version/sha256 on each release (sha from the
# release's SHA256SUMS or `shasum -a 256 <dmg>`).
cask "espresso-macchiato" do
  version "0.2.0"
  sha256 "87f32f6f47d63d5c435233b70a3d083daf3092f97382dfcfbe87d9c58f63f71b"

  url "https://github.com/simiriva95/EspressoMacchiato/releases/download/v#{version}/EspressoMacchiato_#{version}_aarch64.dmg"
  name "EspressoMacchiato"
  desc "Keep your machine awake and present — menu bar keep-awake with a real idle-counter reset"
  homepage "https://github.com/simiriva95/EspressoMacchiato"

  depends_on arch: :arm64
  depends_on macos: ">= :monterey"
  app "EspressoMacchiato.app"

  caveats <<~EOS
    EspressoMacchiato is not notarized yet — install with --no-quarantine:
      brew install --cask --no-quarantine espresso-macchiato
    If you installed without it and macOS says the app is damaged:
      xattr -cr /Applications/EspressoMacchiato.app
    Then grant Accessibility: System Settings → Privacy & Security →
    Accessibility → enable EspressoMacchiato (needed to reset the idle
    counter — the app tells you if it's missing).
  EOS

  zap trash: [
    "~/Library/Application Support/app.espressomacchiato",
  ]
end
