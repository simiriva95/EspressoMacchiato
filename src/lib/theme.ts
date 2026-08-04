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
