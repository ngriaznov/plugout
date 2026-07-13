import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Settings } from "../settings";

interface Props {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
  onClose: () => void;
}

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function SettingsModal({ settings, onChange, onClose }: Props) {
  const [manualDir, setManualDir] = useState("");

  const addDir = (dir: string) => {
    const d = dir.trim();
    if (d && !settings.extraScanDirs.includes(d)) {
      onChange({ extraScanDirs: [...settings.extraScanDirs, d] });
    }
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") addDir(dir);
  };

  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="inspector-head">
          <h2>Settings</h2>
          <button className="x" aria-label="Close settings" onClick={onClose}>✕</button>
        </div>
        <label className="setting-row">
          <input
            type="checkbox"
            checked={settings.usageScan}
            onChange={(e) => onChange({ usageScan: e.target.checked })}
          />
          <span>
            <span className="setting-title">Scan DAW projects for plugin usage</span>
            <span className="setting-sub">
              Finds REAPER and Ableton project files via Spotlight and reads them to
              show the Used column. May trigger macOS folder-access prompts.
            </span>
          </span>
        </label>

        <section className="settings-section">
          <h3>Scan locations</h3>
          <p className="setting-sub">
            Extra folders scanned for plugin bundles, alongside the standard ones.
          </p>
          <ul className="scan-dirs">
            {settings.extraScanDirs.map((d) => (
              <li key={d}>
                <code>{d}</code>
                <button
                  className="ghost small"
                  aria-label={`Remove ${d}`}
                  onClick={() => onChange({ extraScanDirs: settings.extraScanDirs.filter((x) => x !== d) })}
                >
                  ✕
                </button>
              </li>
            ))}
            {settings.extraScanDirs.length === 0 && <li className="setting-sub">None</li>}
          </ul>
          {isTauri ? (
            <button className="ghost small" onClick={pickDir}>
              Add folder…
            </button>
          ) : (
            <form
              className="scan-dir-add"
              onSubmit={(e) => {
                e.preventDefault();
                addDir(manualDir);
                setManualDir("");
              }}
            >
              <input
                aria-label="Folder path"
                placeholder="/path/to/plugins"
                value={manualDir}
                onChange={(e) => setManualDir(e.target.value)}
              />
              <button type="submit" className="ghost small">
                Add
              </button>
            </form>
          )}
        </section>
      </div>
    </div>
  );
}
