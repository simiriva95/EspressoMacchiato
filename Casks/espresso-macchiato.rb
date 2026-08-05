# Homebrew cask, served straight from this repo as a tap:
#   brew tap simiriva95/espressomacchiato https://github.com/simiriva95/EspressoMacchiato
#   brew install --cask espresso-macchiato
#
# The app is self-signed, not notarized, so the postflight strips the
# quarantine flag (Gatekeeper would otherwise reject it). Bump version and
# sha256 on each release (sha from the release's SHA256SUMS or
# `shasum -a 256 <dmg>`).
cask "espresso-macchiato" do
  version "0.3.0"
  sha256 "07daee14c0005891b4dc55c8e04aecad13d8bd7d4216182a4c3228baa8d9f684"

  url "https://github.com/simiriva95/EspressoMacchiato/releases/download/v#{version}/EspressoMacchiato_#{version}_aarch64.dmg"
  name "EspressoMacchiato"
  desc "Keep your machine awake and present — menu bar keep-awake with a real idle-counter reset"
  homepage "https://github.com/simiriva95/EspressoMacchiato"

  depends_on arch: :arm64
  depends_on macos: ">= :monterey"
  app "EspressoMacchiato.app"

  # Self-signed, not notarized: drop the quarantine flag so Gatekeeper lets
  # it launch. The code is public and inspectable; notarization costs
  # 99 $/yr and is skipped for v1.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/EspressoMacchiato.app"],
                   sudo: false
  end

  caveats <<~EOS
    After install, grant Accessibility so EspressoMacchiato can reset the
    idle counter: System Settings → Privacy & Security → Accessibility →
    enable EspressoMacchiato (the app tells you if it's missing).
  EOS

  zap trash: [
    "~/Library/Application Support/app.espressomacchiato",
  ]
end
