/** The app's look: the night-blue one, a plain light grey one, or an old parchment. */
export type Theme = "dark" | "light" | "parchemin";

export const THEMES: { key: Theme; label: string }[] = [
  { key: "dark", label: "Sombre" },
  { key: "light", label: "Clair" },
  { key: "parchemin", label: "Parchemin" },
];

// Kept in the window's storage, not with the settings: read before the first paint (no dark
// flash when a light theme is chosen).
const KEY = "factoruide.theme";

export function savedTheme(): Theme {
  const saved = localStorage.getItem(KEY);
  return THEMES.some((t) => t.key === saved) ? (saved as Theme) : "dark";
}

export function applyTheme(theme: Theme) {
  if (theme === "dark") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  localStorage.setItem(KEY, theme);
}
