import { useEffect, useRef, useState } from "react";
import type { Plugin } from "../types";
import { FormatChip } from "./FormatBadge";
import { prefetchDetails } from "../detailsCache";
import { formatBytes, usageFor, type SortDir, type SortKey, type Usage } from "../util";

interface Props {
  plugins: Plugin[];
  selected: Set<string>;
  loading: boolean;
  query: string;
  inspectedKey?: string;
  sort: { key: SortKey; dir: SortDir };
  onSort: (key: SortKey) => void;
  onTogglePlugin: (p: Plugin) => void;
  onToggleInstall: (id: string) => void;
  onToggleAll: () => void;
  onRowClick: (p: Plugin) => void;
  onClearSearch: () => void;
  related?: Plugin[];
  usage?: Map<string, Usage>;
}

function TriCheckbox({
  checked,
  indeterminate,
  onChange,
  label,
}: {
  checked: boolean;
  indeterminate: boolean;
  onChange: () => void;
  label: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = indeterminate;
  }, [indeterminate]);
  return <input ref={ref} type="checkbox" aria-label={label} checked={checked} onChange={onChange} />;
}

function SortHeader({
  label,
  k,
  sort,
  onSort,
  className,
  title,
}: {
  label: string;
  k: SortKey;
  sort: { key: SortKey; dir: SortDir };
  onSort: (key: SortKey) => void;
  className?: string;
  title?: string;
}) {
  const active = sort.key === k;
  return (
    <th
      className={className}
      title={title}
      aria-sort={active ? (sort.dir === 1 ? "ascending" : "descending") : undefined}
    >
      <button type="button" className="th-btn" onClick={() => onSort(k)}>
        {label}
        <span className="sort-ind">{active ? (sort.dir === 1 ? "↑" : "↓") : ""}</span>
      </button>
    </th>
  );
}

function SkeletonRows({ showUsed }: { showUsed: boolean }) {
  return (
    <>
      {Array.from({ length: 9 }, (_, i) => (
        <tr key={i} className="skel-row" style={{ animationDelay: `${i * 60}ms` }}>
          <td className="c-check" />
          <td>
            <div className="skel" style={{ width: `${46 + ((i * 17) % 34)}%` }} />
          </td>
          <td className="c-vendor"><div className="skel" style={{ width: 70 }} /></td>
          <td><div className="skel" style={{ width: 110 }} /></td>
          <td><div className="skel" style={{ width: 54 }} /></td>
          {showUsed && <td className="c-used" />}
          <td className="c-size"><div className="skel skel-right" style={{ width: 52 }} /></td>
        </tr>
      ))}
    </>
  );
}

export function PluginList(p: Props) {
  const showUsed = p.usage !== undefined;
  const columnCount = showUsed ? 7 : 6;
  const allIds = p.plugins.flatMap((pl) => pl.installs.map((b) => b.id));
  const allChecked = allIds.length > 0 && allIds.every((id) => p.selected.has(id));
  const someChecked = allIds.some((id) => p.selected.has(id));

  // A product can already be visible via one install's spelling while a
  // sibling install of the same product only hits semantically; dedupe by
  // key so it never renders under both the main list and "Related matches".
  const shownKeys = new Set(p.plugins.map((pl) => pl.key));
  const relatedRows = (p.related ?? []).filter((pl) => !shownKeys.has(pl.key));

  const [activeKey, setActiveKey] = useState<string | null>(null);
  const allRows = [...p.plugins, ...relatedRows];
  // A focused row can drop out of the list (search/filter change) while it's
  // still the active key; fall back to the first row so the table stays
  // reachable by keyboard instead of every row going tabIndex=-1.
  const effectiveActiveKey = allRows.some((pl) => pl.key === activeKey) ? activeKey : allRows[0]?.key;

  const onRowKeyDown = (e: React.KeyboardEvent, pl: Plugin, idx: number) => {
    // Keydown bubbles up from the row's own interactive children (format
    // chips, the select checkbox); let those handle Space/Enter natively
    // instead of the row intercepting them.
    if (e.target !== e.currentTarget) return;
    const move = (to: number) => {
      const t = allRows[Math.max(0, Math.min(allRows.length - 1, to))];
      if (t) {
        setActiveKey(t.key);
        document.querySelector<HTMLElement>(`tr[data-key="${CSS.escape(t.key)}"]`)?.focus();
      }
    };
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); move(idx + 1); break;
      case "ArrowUp": e.preventDefault(); move(idx - 1); break;
      case "Home": e.preventDefault(); move(0); break;
      case "End": e.preventDefault(); move(allRows.length - 1); break;
      case " ": e.preventDefault(); p.onTogglePlugin(pl); break;
      case "Enter": e.preventDefault(); p.onRowClick(pl); break;
    }
  };

  const renderRow = (pl: Plugin, idx: number) => {
    const selCount = pl.installs.filter((b) => p.selected.has(b.id)).length;
    return (
      <tr
        key={pl.key}
        className={pl.key === p.inspectedKey ? "sel" : ""}
        onClick={() => p.onRowClick(pl)}
        onMouseEnter={() => prefetchDetails(pl)}
        tabIndex={pl.key === effectiveActiveKey ? 0 : -1}
        data-key={pl.key}
        aria-selected={pl.installs.every((b) => p.selected.has(b.id))}
        onFocus={() => setActiveKey(pl.key)}
        onKeyDown={(e) => onRowKeyDown(e, pl, idx)}
      >
        <td className="c-check" onClick={(e) => e.stopPropagation()}>
          <TriCheckbox
            checked={selCount === pl.installs.length}
            indeterminate={selCount > 0 && selCount < pl.installs.length}
            onChange={() => p.onTogglePlugin(pl)}
            label={`Select ${pl.name}`}
          />
        </td>
        <td className="c-name">
          <div className="name">{pl.name}</div>
          <div className="vendor">{pl.vendor}</div>
        </td>
        <td className="c-vendor">{pl.vendor}</td>
        <td className="c-chips">
          {pl.installs.map((b) => (
            <FormatChip
              key={b.id}
              format={b.format}
              selected={p.selected.has(b.id)}
              onToggle={() => p.onToggleInstall(b.id)}
            />
          ))}
        </td>
        <td className="c-version">{pl.version || "—"}</td>
        {showUsed && (
          <td className="c-used">{(p.usage && usageFor(pl, p.usage)?.projects) ?? "—"}</td>
        )}
        <td className="c-size">{formatBytes(pl.sizeBytes)}</td>
      </tr>
    );
  };

  return (
    <table className="table">
      <thead>
        <tr>
          <th className="c-check">
            <TriCheckbox
              checked={allChecked}
              indeterminate={!allChecked && someChecked}
              onChange={p.onToggleAll}
              label="Select all plugins"
            />
          </th>
          <SortHeader label="Plugin" k="name" sort={p.sort} onSort={p.onSort} />
          <SortHeader label="Vendor" k="vendor" sort={p.sort} onSort={p.onSort} className="c-vendor-h" />
          <SortHeader label="Formats" k="formats" sort={p.sort} onSort={p.onSort} className="c-chips-h" />
          <SortHeader label="Version" k="version" sort={p.sort} onSort={p.onSort} className="c-version-h" />
          {showUsed && (
            <SortHeader
              label="Used"
              k="used"
              sort={p.sort}
              onSort={p.onSort}
              className="c-used-h"
              title="Projects referencing this plugin (REAPER, Ableton, Studio One and Logic Pro files scanned)"
            />
          )}
          <SortHeader label="Size" k="size" sort={p.sort} onSort={p.onSort} className="c-size" />
        </tr>
      </thead>
      <tbody>
        {p.loading && p.plugins.length === 0 && <SkeletonRows showUsed={showUsed} />}
        {p.plugins.map((pl, idx) => renderRow(pl, idx))}
        {relatedRows.length > 0 && (
          <>
            {/* Shaped like a data row (no colSpan): a spanning cell makes
                table-layout:fixed hand width back to container-query-hidden
                columns, starving the name column on narrow windows. */}
            <tr className="related-divider">
              <td className="c-check" />
              <td>Related matches</td>
              <td className="c-vendor" />
              <td className="c-chips" />
              <td className="c-version" />
              {showUsed && <td className="c-used" />}
              <td className="c-size" />
            </tr>
            {relatedRows.map((pl, i) => renderRow(pl, p.plugins.length + i))}
          </>
        )}
        {!p.loading && p.plugins.length === 0 && relatedRows.length === 0 && (
          <tr>
            <td colSpan={columnCount} className="empty">
              <div className="empty-state">
                <div className="empty-title">
                  {p.query ? <>No plugins match “{p.query}”</> : "No plugins found"}
                </div>
                {p.query && (
                  <button className="ghost" onClick={p.onClearSearch}>Clear search</button>
                )}
              </div>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
