import type { PluginBundle } from "../types";
import { formatBytes, mergePlugins } from "../util";
import { FormatBadge } from "./FormatBadge";

interface Props { bundles: PluginBundle[]; onCancel: () => void; onConfirm: () => void; busy: boolean; }

export function ConfirmModal({ bundles, onCancel, onConfirm, busy }: Props) {
  const plugins = mergePlugins(bundles);
  const total = bundles.reduce((n, b) => n + b.sizeBytes, 0);
  const hasSystem = bundles.some((b) => b.scope === "system");
  return (
    <div className="overlay" onClick={onCancel}>
      <div className="modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2>Move {plugins.length} plugin{plugins.length > 1 ? "s" : ""} to Trash?</h2>
        <p className="dim">
          Frees about {formatBytes(total)}. Everything goes to the Trash first, so you can
          restore it if needed.
        </p>
        <ul className="confirm-list">
          {plugins.map((p) => (
            <li key={p.key}>
              <span className="confirm-name">{p.name}</span>
              <span className="confirm-formats">
                {p.installs.map((b) => <FormatBadge key={b.id} format={b.format} />)}
              </span>
              <span className="confirm-size">{formatBytes(p.sizeBytes)}</span>
            </li>
          ))}
        </ul>
        {hasSystem && <p className="warn">System plugins require an administrator password.</p>}
        <div className="modal-actions">
          <button className="ghost" onClick={onCancel} disabled={busy}>Cancel</button>
          <button className="danger" onClick={onConfirm} disabled={busy}>
            {busy ? "Removing…" : "Move to Trash"}
          </button>
        </div>
      </div>
    </div>
  );
}
