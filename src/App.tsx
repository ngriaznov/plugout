import { useEffect, useMemo, useRef, useState } from "react";
import type { Format, Scope, Plugin, PluginBundle, RemovalResult } from "./types";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { startScan, removeItems, onScanBatch, onScanDone, onReceiptUpdate } from "./api";
import { clearDetailsCache } from "./detailsCache";
import { applyTheme, getPref, setPref, onSystemThemeChange, type ThemePref } from "./theme";
import { checkForUpdate, downloadAndInstall, restartApp, type UpdateState } from "./updater";
import { mergePlugins, sortPlugins, type SortDir, type SortKey } from "./util";
import { Sidebar } from "./components/Sidebar";
import { PluginList } from "./components/PluginList";
import { Inspector } from "./components/Inspector";
import { ActionBar } from "./components/ActionBar";
import { ConfirmModal } from "./components/ConfirmModal";
import { UpdatePill } from "./components/UpdatePill";

export default function App() {
  const [bundles, setBundles] = useState<PluginBundle[]>([]);
  const [loading, setLoading] = useState(true);
  const [formatFilter, setFormat] = useState<Format | "ALL">("ALL");
  const [scopeFilter, setScope] = useState<Scope | "ALL">("ALL");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [inspectedKey, setInspectedKey] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<RemovalResult[] | null>(null);
  const [themePref, setThemePref] = useState<ThemePref>(getPref);
  const [sort, setSort] = useState<{ key: SortKey; dir: SortDir }>({ key: "name", dir: 1 });
  const [update, setUpdate] = useState<UpdateState>({ phase: "idle" });
  const toastTimer = useRef<number | null>(null);

  // Quiet update check once the launch dust settles; failures stay silent —
  // an unreachable update endpoint should never bother the user.
  useEffect(() => {
    const t = window.setTimeout(() => {
      checkForUpdate()
        .then((version) => version && setUpdate({ phase: "available", version }))
        .catch(() => {});
    }, 3000);
    return () => clearTimeout(t);
  }, []);

  async function startUpdate() {
    if (update.phase !== "available") return;
    const { version } = update;
    setUpdate({ phase: "downloading", version, percent: null });
    try {
      await downloadAndInstall((percent) => setUpdate({ phase: "downloading", version, percent }));
      setUpdate({ phase: "ready", version });
    } catch (e) {
      setUpdate({ phase: "error", message: String(e) });
    }
  }

  const changeSort = (key: SortKey) =>
    setSort((s) =>
      s.key === key
        ? { key, dir: -s.dir as SortDir }
        : { key, dir: key === "size" ? -1 : 1 }, // size starts biggest-first
    );

  useEffect(() => {
    applyTheme(themePref);
    // Re-resolve "Auto" when macOS switches appearance.
    return onSystemThemeChange(() => applyTheme(getPref()));
  }, [themePref]);

  function changeTheme(pref: ThemePref) {
    setPref(pref);
    setThemePref(pref);
  }

  // Clear state and kick off a fresh streaming scan. Results arrive via the
  // listeners registered on mount, so the UI populates progressively.
  function rescan() {
    clearDetailsCache();
    setBundles([]);
    setSelected(new Set());
    setInspectedKey(null);
    setLoading(true);
    startScan();
  }

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    let disposed = false;
    (async () => {
      const subs = await Promise.all([
        onScanBatch((batch) => setBundles((prev) => [...prev, ...batch])),
        onScanDone(() => setLoading(false)),
        onReceiptUpdate((updates) =>
          setBundles((prev) => {
            const m = new Map(updates.map((u) => [u.id, u.packageId]));
            return prev.map((b) => (m.has(b.id) ? { ...b, packageId: m.get(b.id) ?? null } : b));
          })),
      ]);
      if (disposed) {
        subs.forEach((u) => u());
        return;
      }
      unlisteners.push(...subs);
      rescan();
    })();
    return () => {
      disposed = true;
      unlisteners.forEach((u) => u());
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (confirming) setConfirming(false);
      else setInspectedKey(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [confirming]);

  async function doRemove(extraPaths: string[]) {
    setBusy(true);
    const res = await removeItems([...selected, ...extraPaths]);
    setBusy(false);
    setConfirming(false);
    setResults(res);
    rescan();
    if (toastTimer.current) clearTimeout(toastTimer.current);
    const hasFailures = res.some((r) => r.status === "failed");
    toastTimer.current = window.setTimeout(() => setResults(null), hasFailures ? 10000 : 6000);
  }

  const visible = useMemo(
    () =>
      bundles.filter(
        (b) =>
          (formatFilter === "ALL" || b.format === formatFilter) &&
          (scopeFilter === "ALL" || b.scope === scopeFilter) &&
          (query === "" || `${b.name} ${b.vendor}`.toLowerCase().includes(query.toLowerCase())),
      ),
    [bundles, formatFilter, scopeFilter, query],
  );

  const plugins = useMemo(
    () => sortPlugins(mergePlugins(visible), sort.key, sort.dir),
    [visible, sort],
  );
  const allPluginCount = useMemo(() => mergePlugins(bundles).length, [bundles]);
  const inspected = plugins.find((p) => p.key === inspectedKey) ?? null;

  const toggleInstall = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const togglePlugin = (p: Plugin) =>
    setSelected((s) => {
      const n = new Set(s);
      const ids = p.installs.map((b) => b.id);
      if (ids.every((id) => n.has(id))) ids.forEach((id) => n.delete(id));
      else ids.forEach((id) => n.add(id));
      return n;
    });

  const toggleAll = () =>
    setSelected((s) =>
      visible.every((b) => s.has(b.id))
        ? new Set([...s].filter((id) => !visible.some((b) => b.id === id)))
        : new Set([...s, ...visible.map((b) => b.id)]),
    );

  const selectedBundles = bundles.filter((b) => selected.has(b.id));
  const selectedPluginCount = new Set(selectedBundles.map((b) => `${b.vendor} ${b.name}`)).size;
  const reclaimable = selectedBundles.reduce((n, b) => n + b.sizeBytes, 0);
  const failed = results?.filter((r) => r.status === "failed") ?? [];

  return (
    <div className="app">
      <Sidebar
        bundles={bundles}
        pluginCount={allPluginCount}
        loading={loading}
        formatFilter={formatFilter}
        onFormat={setFormat}
        scopeFilter={scopeFilter}
        onScope={setScope}
        query={query}
        onQuery={setQuery}
        themePref={themePref}
        onTheme={changeTheme}
      />

      <main className="main">
        <header className="toolbar">
          <div className="count">
            {loading ? (
              <>
                <span className="spinner" /> Scanning… {bundles.length} found
              </>
            ) : (
              `${plugins.length} plugins`
            )}
          </div>
          <div className="spacer" />
          <UpdatePill state={update} onDownload={startUpdate} onRestart={restartApp} />
          <button className="ghost small" onClick={rescan} disabled={loading}>
            Rescan
          </button>
        </header>

        <div className="body">
          <div className="table-wrap">
            <PluginList
              plugins={plugins}
              selected={selected}
              loading={loading}
              query={query}
              inspectedKey={inspected?.key}
              sort={sort}
              onSort={changeSort}
              onTogglePlugin={togglePlugin}
              onToggleInstall={toggleInstall}
              onToggleAll={toggleAll}
              onRowClick={(p) => setInspectedKey((k) => (k === p.key ? null : p.key))}
              onClearSearch={() => setQuery("")}
            />
          </div>
          {inspected && <Inspector plugin={inspected} onClose={() => setInspectedKey(null)} />}
        </div>

        {selected.size > 0 && !results && (
          <ActionBar
            pluginCount={selectedPluginCount}
            installCount={selected.size}
            sizeBytes={reclaimable}
            onClear={() => setSelected(new Set())}
            onRemove={() => setConfirming(true)}
          />
        )}

        {confirming && (
          <ConfirmModal
            bundles={selectedBundles}
            allBundles={bundles}
            busy={busy}
            onCancel={() => setConfirming(false)}
            onConfirm={doRemove}
          />
        )}

        {results && (
          <div className={`toast${failed.length > 0 ? " toast-error" : ""}`} role="status">
            <strong>{results.filter((r) => r.status === "trashed").length} moved to Trash</strong>
            {failed.length > 0 && (
              <span className="toast-fail">
                {" "}· {failed.length} failed{failed[0]?.message ? ` — ${failed[0].message}` : ""}
              </span>
            )}
          </div>
        )}
      </main>
    </div>
  );
}
