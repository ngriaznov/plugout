import type { Format, Scope, PluginBundle } from "../types";
import { FORMATS } from "../types";
import { formatBytes } from "../util";
import { SizeChart } from "./SizeChart";

interface Props {
  bundles: PluginBundle[];
  pluginCount: number;
  loading: boolean;
  formatFilter: Format | "ALL";
  onFormat: (f: Format | "ALL") => void;
  scopeFilter: Scope | "ALL";
  onScope: (s: Scope | "ALL") => void;
  query: string;
  onQuery: (q: string) => void;
}

export function Sidebar(p: Props) {
  const formatCount = (f: Format) => p.bundles.filter((b) => b.format === f).length;
  const scopeCount = (s: Scope) => p.bundles.filter((b) => b.scope === s).length;
  const totalSize = p.bundles.reduce((n, b) => n + b.sizeBytes, 0);

  return (
    <aside className="sidebar">
      <div className="brand">plug<span>out</span></div>

      <div className="search-wrap" role="search">
        <svg className="search-icon" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="7" cy="7" r="4.6" fill="none" stroke="currentColor" strokeWidth="1.5" />
          <line x1="10.4" y1="10.4" x2="13.6" y2="13.6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        <input
          id="plugin-search"
          className="search"
          placeholder="Search plugins…"
          aria-label="Search plugins"
          value={p.query}
          onChange={(e) => p.onQuery(e.target.value)}
        />
        {p.query && (
          <button className="search-clear" aria-label="Clear search" onClick={() => p.onQuery("")}>
            ✕
          </button>
        )}
      </div>

      <nav aria-label="Filters">
        <div className="group-label">Format</div>
        <button className={`filter ${p.formatFilter === "ALL" ? "on" : ""}`} onClick={() => p.onFormat("ALL")}>
          <span>All</span><span className="pill">{p.bundles.length}</span>
        </button>
        {FORMATS.map((f) => (
          <button key={f} className={`filter ${p.formatFilter === f ? "on" : ""}`} onClick={() => p.onFormat(f)}>
            <span>{f}</span><span className="pill">{formatCount(f)}</span>
          </button>
        ))}

        <div className="group-label">Location</div>
        {(["ALL", "user", "system"] as const).map((s) => (
          <button key={s} className={`filter ${p.scopeFilter === s ? "on" : ""}`} onClick={() => p.onScope(s)}>
            <span>{s === "ALL" ? "All" : s === "user" ? "User" : "System"}</span>
            {s !== "ALL" && <span className="pill">{scopeCount(s)}</span>}
          </button>
        ))}
      </nav>

      <SizeChart bundles={p.bundles} />

      <footer className="sidebar-foot">
        {p.loading
          ? "Scanning…"
          : `${p.pluginCount} plugins · ${formatBytes(totalSize)}`}
      </footer>
    </aside>
  );
}
