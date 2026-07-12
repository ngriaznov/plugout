export interface Settings {
  usageScan: boolean;
}

const KEY = "plugout:settings";
const DEFAULTS: Settings = { usageScan: false };

export function getSettings(): Settings {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { usageScan: parsed.usageScan === true };
  } catch {
    return { ...DEFAULTS };
  }
}

export function setSettings(patch: Partial<Settings>): Settings {
  const next = { ...getSettings(), ...patch };
  localStorage.setItem(KEY, JSON.stringify(next));
  return next;
}
