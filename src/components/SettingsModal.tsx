import type { Settings } from "../settings";

interface Props {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
  onClose: () => void;
}

export function SettingsModal({ settings, onChange, onClose }: Props) {
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
      </div>
    </div>
  );
}
