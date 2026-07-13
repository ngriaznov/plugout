export interface Settings {
  usageScan: boolean;
  extraScanDirs: string[];
}

const KEY = "plugout:settings";
const DEFAULTS: Settings = { usageScan: false, extraScanDirs: [] };

export function getSettings(): Settings {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      usageScan: parsed.usageScan === true,
      extraScanDirs: Array.isArray(parsed.extraScanDirs)
        ? parsed.extraScanDirs.filter((d: unknown): d is string => typeof d === "string")
        : [],
    };
  } catch {
    return { ...DEFAULTS };
  }
}

export function setSettings(patch: Partial<Settings>): Settings {
  const next = { ...getSettings(), ...patch };
  localStorage.setItem(KEY, JSON.stringify(next));
  return next;
}
