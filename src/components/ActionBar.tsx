import { formatBytes } from "../util";

interface Props {
  pluginCount: number;
  installCount: number;
  sizeBytes: number;
  onClear: () => void;
  onRemove: () => void;
}

export function ActionBar(p: Props) {
  return (
    <div className="actionbar" role="toolbar" aria-label="Selection actions">
      <div className="actionbar-info">
        <strong>
          {p.pluginCount} plugin{p.pluginCount === 1 ? "" : "s"}
        </strong>
        <span className="dim">
          {p.installCount !== p.pluginCount && <>{p.installCount} installs · </>}
          {formatBytes(p.sizeBytes)} reclaimable
        </span>
      </div>
      <button className="ghost" onClick={p.onClear}>Clear</button>
      <button className="danger" onClick={p.onRemove}>Remove…</button>
    </div>
  );
}
