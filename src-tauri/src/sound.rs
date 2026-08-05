//! Optional steam hiss on activation. The sample is synthesized once
//! (decaying band-limited noise ≈ espresso machine steam wand) into a temp
//! WAV — no bundled audio assets — and played with the OS player.

use std::path::PathBuf;
use std::sync::OnceLock;

const SAMPLE_RATE: u32 = 22_050;
const DURATION_SECS: f64 = 0.4;

fn synthesize_hiss() -> Vec<u8> {
    let samples = (SAMPLE_RATE as f64 * DURATION_SECS) as usize;
    let mut data = Vec::with_capacity(44 + samples * 2);

    // WAV header (PCM 16-bit mono).
    let byte_len = (samples * 2) as u32;
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + byte_len).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&1u16.to_le_bytes()); // mono
    data.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    data.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&byte_len.to_le_bytes());

    // Deterministic xorshift noise → no rand dependency.
    let mut seed: u32 = 0xC0FFEE;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f64 / u32::MAX as f64) * 2.0 - 1.0
    };

    let mut low_pass = 0.0f64;
    for i in 0..samples {
        let t = i as f64 / SAMPLE_RATE as f64;
        // Quick attack, exponential decay; gentle low-pass keeps it soft.
        let envelope = (t * 120.0).min(1.0) * (-t * 9.0).exp();
        low_pass += 0.35 * (noise() - low_pass);
        let value = (low_pass * envelope * 0.5 * f64::from(i16::MAX)) as i16;
        data.extend_from_slice(&value.to_le_bytes());
    }
    data
}

fn hiss_path() -> &'static Option<PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = std::env::temp_dir().join("espresso-macchiato-steam.wav");
        std::fs::write(&path, synthesize_hiss()).ok()?;
        Some(path)
    })
}

/// Fire-and-forget playback; missing players fail silently.
pub fn play_steam() {
    let Some(path) = hiss_path() else { return };
    #[cfg(target_os = "macos")]
    let players: &[&str] = &["afplay"];
    #[cfg(not(target_os = "macos"))]
    let players: &[&str] = &["paplay", "aplay"];

    for player in players {
        if std::process::Command::new(player)
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return;
        }
    }
}
