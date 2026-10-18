/** The app's look: the system's (dark or light), the night-blue one, its light version (the same
 * sky by day), or an old parchment. */
export type Theme = "auto" | "dark" | "light" | "parchemin";

export const THEMES: { key: Theme; label: string }[] = [
  { key: "auto", label: "Automatique (système)" },
  { key: "dark", label: "Sombre" },
  { key: "light", label: "Clair" },
  { key: "parchemin", label: "Parchemin" },
];

// Kept in the window's storage, not with the settings: read before the first paint (no dark
// flash when a light theme is chosen).
const KEY = "factoruide.theme";

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

export function savedTheme(): Theme {
  const saved = localStorage.getItem(KEY);
  return THEMES.some((t) => t.key === saved) ? (saved as Theme) : "dark";
}

function show(theme: Theme) {
  const shown = theme === "auto" ? (systemDark.matches ? "dark" : "light") : theme;
  if (shown === "dark") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = shown;
}

export function applyTheme(theme: Theme) {
  show(theme);
  localStorage.setItem(KEY, theme);
}

// The system switching between dark and light while the app is open.
systemDark.addEventListener("change", () => {
  if (savedTheme() === "auto") show("auto");
});
