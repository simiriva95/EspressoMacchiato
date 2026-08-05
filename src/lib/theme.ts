/** Apply the theme preference: explicit light/dark pins data-theme, the
 * "system" value removes it so prefers-color-scheme decides. */
export function applyTheme(theme: string) {
  const root = document.documentElement;
  if (theme === "light" || theme === "dark") {
    root.dataset.theme = theme;
  } else {
    delete root.dataset.theme;
  }
}

/** Accent swatches: hex → readable on-accent text color derives from
 * luminance, so any custom hex stays accessible. */
export const ACCENT_SWATCHES = [
  "#e8a54c", // caramel (default)
  "#7bc47f", // matcha
  "#a78bfa", // violet
  "#60a5fa", // sky
  "#fb7185", // rose
];

export function applyAccent(accent: string) {
  if (!/^#[0-9a-fA-F]{6}$/.test(accent)) return;
  const root = document.documentElement;
  root.style.setProperty("--accent", accent);
  root.style.setProperty(
    "--on-accent",
    luminance(accent) > 0.45 ? "#201403" : "#ffffff",
  );
}

function luminance(hex: string): number {
  const channel = (i: number) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}
