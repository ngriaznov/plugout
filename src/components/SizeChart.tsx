import type { PluginBundle } from "../types";
import { FORMATS } from "../types";
import { formatBytes } from "../util";

interface Props {
  bundles: PluginBundle[];
}

/** Disk-size composition by format: one stacked bar plus a legend that
 * doubles as the value table (identity and size are never color-alone).
 * Slot colors are fixed per format — see the --viz-* tokens in styles.css. */
export function SizeChart(p: Props) {
  const total = p.bundles.reduce((n, b) => n + b.sizeBytes, 0);
  if (total === 0) return null;

  const slices = FORMATS.map((format) => ({
    format,
    bytes: p.bundles.filter((b) => b.format === format).reduce((n, b) => n + b.sizeBytes, 0),
  })).filter((s) => s.bytes > 0);

  return (
    <div className="size-chart">
      <div className="group-label">Size</div>
      <div className="size-bar" role="img" aria-label={`Disk use by format, ${formatBytes(total)} total`}>
        {slices.map((s) => (
          <span
            key={s.format}
            className="size-seg"
            style={{ flexGrow: s.bytes, background: `var(--viz-${s.format.toLowerCase()})` }}
            title={`${s.format} — ${formatBytes(s.bytes)} (${Math.round((s.bytes / total) * 100)}%)`}
          />
        ))}
      </div>
      <ul className="size-legend">
        {slices.map((s) => (
          <li key={s.format}>
            <span className="size-chip" style={{ background: `var(--viz-${s.format.toLowerCase()})` }} aria-hidden="true" />
            <span className="size-name">{s.format}</span>
            <span className="size-val">{formatBytes(s.bytes)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
