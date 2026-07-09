export type ThemePref = "light" | "dark" | "system";

const KEY = "plugout-theme";
const mq = window.matchMedia("(prefers-color-scheme: dark)");

export function getPref(): ThemePref {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" ? v : "system";
}

function resolve(pref: ThemePref): "light" | "dark" {
  return pref === "system" ? (mq.matches ? "dark" : "light") : pref;
}

// Mirrors the pre-paint script in index.html — keep the two in sync.
export function applyTheme(pref: ThemePref): void {
  const theme = resolve(pref);
  const el = document.documentElement;
  el.dataset.theme = theme;
  el.style.background = theme === "dark" ? "#121110" : "#e9e7e2";
  el.style.colorScheme = theme;
}

export function setPref(pref: ThemePref): void {
  localStorage.setItem(KEY, pref);
  applyTheme(pref);
}

export function onSystemThemeChange(cb: () => void): () => void {
  mq.addEventListener("change", cb);
  return () => mq.removeEventListener("change", cb);
}
