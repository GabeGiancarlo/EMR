export const THEME_STORAGE_KEY = "nexus.theme";

export const THEME_MODES = ["light", "dark", "system"] as const;

export type ThemeMode = (typeof THEME_MODES)[number];

export type ResolvedTheme = "light" | "dark";

const DARK_CANVAS = "#161A13";      // --nx-page, dark (Night)
const LIGHT_CANVAS = "#F4EFE3";     // --nx-page, light (Linen)
const LIGHT_THEME_COLOR = "#2F4631"; // Pine

export function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

export function readStoredTheme(): ThemeMode {
  if (typeof localStorage === "undefined") return "system";
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeMode(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

export function persistTheme(mode: ThemeMode): void {
  if (!isThemeMode(mode) || typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, mode);
  } catch {
    /* Preference still applies for this view when storage is blocked. */
  }
}

export function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function resolveTheme(mode: ThemeMode, osDark = systemPrefersDark()): ResolvedTheme {
  if (mode === "dark") return "dark";
  if (mode === "light") return "light";
  return osDark ? "dark" : "light";
}

export function themeAfterOsChange(mode: ThemeMode, osDark: boolean): {
  mode: ThemeMode;
  resolved: ResolvedTheme;
} {
  return { mode, resolved: resolveTheme(mode, osDark) };
}

export function applyTheme(mode: ThemeMode, osDark = systemPrefersDark()): ResolvedTheme {
  const resolved = resolveTheme(mode, osDark);
  if (typeof document === "undefined") return resolved;
  const root = document.documentElement;
  root.setAttribute("data-theme", mode);
  root.setAttribute("data-resolved-theme", resolved);
  root.setAttribute("data-nexus-theme", resolved); // switches the --nx-* role tokens
  root.style.colorScheme = resolved;
  root.style.backgroundColor = resolved === "dark" ? DARK_CANVAS : LIGHT_CANVAS;
  root.style.color = resolved === "dark" ? "#EEEDE0" : "#232B1E";
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", resolved === "dark" ? DARK_CANVAS : LIGHT_THEME_COLOR);
  return resolved;
}

export function watchSystemTheme(onChange: (osDark: boolean) => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return () => {};
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const listener = (event: MediaQueryListEvent) => onChange(event.matches);
  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}
