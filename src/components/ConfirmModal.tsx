import { useEffect, useMemo, useState } from "react";
import type { PluginBundle, RemovalPreview } from "../types";
import { removalPreview } from "../api";
import { formatBytes, mergePlugins } from "../util";
import { FormatBadge } from "./FormatBadge";

interface Props {
  bundles: PluginBundle[];
  /** The full scanned set — exclusivity of support files is judged against it. */
  allBundles: PluginBundle[];
  onCancel: () => void;
  /** Called with any support-file paths the user kept toggled on. */
  onConfirm: (extraPaths: string[]) => void;
  busy: boolean;
}

export function ConfirmModal({ bundles, allBundles, onCancel, onConfirm, busy }: Props) {
  const plugins = useMemo(() => mergePlugins(bundles), [bundles]);
  const total = bundles.reduce((n, b) => n + b.sizeBytes, 0);
  const hasSystem = bundles.some((b) => b.scope === "system");

  const [preview, setPreview] = useState<RemovalPreview | null>(null);
  const [includeSupport, setIncludeSupport] = useState(true);
  const [showFiles, setShowFiles] = useState(false);

  useEffect(() => {
    let active = true;
    removalPreview(bundles.map((b) => b.id), allBundles).then(
      (p) => active && setPreview(p),
      () => active && setPreview({ supportFiles: [], skippedShared: 0 }),
    );
    return () => {
      active = false;
    };
    // bundle identity is stable for the modal's lifetime
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const supportSize = preview?.supportFiles.reduce((n, f) => n + f.sizeBytes, 0) ?? 0;
  const extraPaths =
    includeSupport && preview ? preview.supportFiles.map((f) => f.path) : [];

  return (
    <div className="overlay" onClick={onCancel}>
      <div className="modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2>Move {plugins.length} item{plugins.length > 1 ? "s" : ""} to Trash?</h2>
        <p className="dim">
          Frees about {formatBytes(total + (includeSupport ? supportSize : 0))}. Everything goes
          to the Trash first, so you can restore it if needed.
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

        {preview === null && (
          <div className="support-section dim" role="status">
            <span className="spinner" /> Checking for support files…
          </div>
        )}
        {preview !== null && preview.supportFiles.length > 0 && (
          <div className="support-section">
            <label className="support-toggle">
              <input
                type="checkbox"
                checked={includeSupport}
                onChange={(e) => setIncludeSupport(e.target.checked)}
              />
              Also remove {preview.supportFiles.length} support item
              {preview.supportFiles.length > 1 ? "s" : ""} · {formatBytes(supportSize)}
              <button className="linkish" onClick={() => setShowFiles((s) => !s)}>
                {showFiles ? "hide" : "show"}
              </button>
            </label>
            {showFiles && (
              <ul className="filelist support-files">
                {preview.supportFiles.map((f) => (
                  <li key={f.path}>
                    {f.path} <span className="dim">· {formatBytes(f.sizeBytes)}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
        {preview !== null && preview.skippedShared > 0 && (
          <p className="dim support-note">
            Some support files were kept — {preview.skippedShared} installer package
            {preview.skippedShared > 1 ? "s are" : " is"} shared with plugins staying installed.
          </p>
        )}

        {hasSystem && <p className="warn">System items require an administrator password.</p>}
        <div className="modal-actions">
          <button className="ghost" onClick={onCancel} disabled={busy}>Cancel</button>
          <button className="danger" onClick={() => onConfirm(extraPaths)} disabled={busy}>
            {busy ? "Removing…" : "Move to Trash"}
          </button>
        </div>
      </div>
    </div>
  );
}
