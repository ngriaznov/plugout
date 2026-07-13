import { useEffect, useMemo, useRef, useState } from "react";
import type { Format, Plugin, Scope, PluginBundle, RemovalResult } from "./types";
import { CATEGORY_LABELS } from "./types";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  startScan,
  removeItems,
  onScanBatch,
  onScanDone,
  onReceiptUpdate,
  onEnrichDone,
  indexSearch,
  saveExport,
  revealInFinder,
  scanUsage,
  type UsageHit,
} from "./api";
import { exportCsv, exportJson } from "./export";
import { clearDetailsCache } from "./detailsCache";
import { applyTheme, getPref, setPref, onSystemThemeChange, type ThemePref } from "./theme";
import { checkForUpdate, downloadAndInstall, restartApp, type UpdateState } from "./updater";
import { getSettings, setSettings, type Settings } from "./settings";
import { fmtDate, matchUsage, mergePlugins, sortPlugins, usageFor, type SortDir, type SortKey } from "./util";
import { useSelection } from "./useSelection";
import { useSemanticRelated } from "./useSemanticRelated";
import { Sidebar } from "./components/Sidebar";
import { PluginList } from "./components/PluginList";
import { Inspector } from "./components/Inspector";
import { ActionBar } from "./components/ActionBar";
import { ConfirmModal } from "./components/ConfirmModal";
import { SettingsModal } from "./components/SettingsModal";
import { ExportModal } from "./components/ExportModal";
import { UpdatePill } from "./components/UpdatePill";

export default function App() {
  const [bundles, setBundles] = useState<PluginBundle[]>([]);
  const [loading, setLoading] = useState(true);
  const [enriching, setEnriching] = useState(true);
  const [formatFilter, setFormat] = useState<Format | "ALL">("ALL");
  const [scopeFilter, setScope] = useState<Scope | "ALL">("ALL");
  const [query, setQuery] = useState("");
  const [inspectedKey, setInspectedKey] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [settings, setSettingsState] = useState<Settings>(getSettings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [exportChoice, setExportChoice] = useState(false);
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<RemovalResult[] | null>(null);
  const [exportedDir, setExportedDir] = useState<string | null>(null);
  const [themePref, setThemePref] = useState<ThemePref>(getPref);
  const [sort, setSort] = useState<{ key: SortKey; dir: SortDir }>({ key: "name", dir: 1 });
  const [update, setUpdate] = useState<UpdateState>({ phase: "idle" });
  const [usageHits, setUsageHits] = useState<UsageHit[]>([]);
  const toastTimer = useRef<number | null>(null);
  const exportToastTimer = useRef<number | null>(null);
  const scannedDirs = useRef<string[]>(getSettings().extraScanDirs);

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
        : { key, dir: key === "size" || key === "used" ? -1 : 1 }, // size/used start biggest-first
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

  function changeSettings(patch: Partial<Settings>) {
    const next = setSettings(patch);
    setSettingsState(next);
    if (patch.usageScan === false && sort.key === "used") {
      setSort({ key: "name", dir: 1 });
    }
  }

  // Clear state and kick off a fresh streaming scan. Results arrive via the
  // listeners registered on mount, so the UI populates progressively.
  function rescan() {
    clearDetailsCache();
    setBundles([]);
    clear();
    setInspectedKey(null);
    setLoading(true);
    setEnriching(true);
    const dirs = getSettings().extraScanDirs;
    scannedDirs.current = dirs;
    startScan(dirs);
  }

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    let disposed = false;
    (async () => {
      const subs = await Promise.all([
        onScanBatch((batch) => setBundles((prev) => [...prev, ...batch])),
        onScanDone(() => setLoading(false)),
        onEnrichDone(() => setEnriching(false)),
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
      // Closest-first: the settings dialog sits on top of everything else,
      // so it dismisses before the confirm modal, which dismisses before
      // the export choice, which dismisses before the inspector panel
      // underneath.
      if (settingsOpen) setSettingsOpen(false);
      else if (confirming) setConfirming(false);
      else if (exportChoice) setExportChoice(false);
      else setInspectedKey(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settingsOpen, confirming, exportChoice]);

  // Feed the semantic-search index once a scan settles. Failures are logged
  // and swallowed — search degrades to substring-only.
  useEffect(() => {
    if (loading || bundles.length === 0) return;
    const docs = bundles.map((b) => ({
      id: b.id,
      name: b.name,
      vendor: b.vendor,
      category: b.category ? CATEGORY_LABELS[b.category] : "",
    }));
    indexSearch(docs).catch((e) => console.warn("semantic index failed", e));
    if (settings.usageScan) {
      scanUsage([...new Set(bundles.map((b) => b.name))]).then(setUsageHits, (e) => {
        console.warn("usage scan failed", e);
        setUsageHits([]);
      });
    } else {
      setUsageHits([]);
    }
    // Re-index only when a scan completes, not on receipt enrichment churn.
    // Toggling the usage setting also re-runs this effect (and therefore the
    // re-index) by design, so the usage scan can start/stop immediately.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, settings.usageScan]);

  async function doRemove(extraPaths: string[]) {
    setBusy(true);
    const res = await removeItems([...selected, ...extraPaths]);
    setBusy(false);
    setConfirming(false);
    const canceled = res.filter((r) => r.status === "canceled");
    const acted = res.filter((r) => r.status !== "canceled");
    // Whole batch canceled at the admin prompt: silent no-op, selection kept.
    if (acted.length === 0 && canceled.length > 0) return;
    setResults(acted);
    setExportedDir(null);
    if (exportToastTimer.current) clearTimeout(exportToastTimer.current);
    rescan();
    if (toastTimer.current) clearTimeout(toastTimer.current);
    const hasFailures = acted.some((r) => r.status === "failed");
    toastTimer.current = window.setTimeout(() => setResults(null), hasFailures ? 10000 : 6000);
  }

  function requestExport() {
    if (selected.size > 0) setExportChoice(true);
    else void doExport(plugins);
  }

  async function doExport(list: Plugin[]) {
    setExportChoice(false);
    const stamp = fmtDate(Date.now());
    try {
      const dir = await saveExport([
        { name: `plugout-inventory-${stamp}.csv`, contents: exportCsv(list) },
        { name: `plugout-inventory-${stamp}.json`, contents: exportJson(list) },
      ]);
      setExportedDir(dir);
    } catch {
      setExportedDir("error");
    }
    if (exportToastTimer.current) clearTimeout(exportToastTimer.current);
    exportToastTimer.current = window.setTimeout(() => setExportedDir(null), 6000);
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

  const related = useSemanticRelated(bundles, query, formatFilter, scopeFilter);

  const usage = useMemo(() => matchUsage(mergePlugins(bundles), usageHits), [bundles, usageHits]);

  const plugins = useMemo(
    () => sortPlugins(mergePlugins(visible), sort.key, sort.dir, usage),
    [visible, sort, usage],
  );
  const allPluginCount = useMemo(() => mergePlugins(bundles).length, [bundles]);
  const inspected =
    plugins.find((p) => p.key === inspectedKey) ?? related.find((p) => p.key === inspectedKey) ?? null;

  const {
    selected,
    setSelected,
    toggleInstall,
    togglePlugin,
    toggleAll,
    clear,
    selectedBundles,
    selectedPluginCount,
    reclaimable,
  } = useSelection(bundles, visible);
  const failed = results?.filter((r) => r.status === "failed") ?? [];

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region />
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
              <>
                {plugins.length} plugins
                {enriching && (
                  <span className="enriching" role="status">
                    <span className="spinner" /> linking installers…
                  </span>
                )}
              </>
            )}
          </div>
          <div className="spacer" />
          <UpdatePill state={update} onDownload={startUpdate} onRestart={restartApp} />
          <button className="ghost small" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
          <button className="ghost small" onClick={requestExport} disabled={loading || bundles.length === 0}>
            Export
          </button>
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
              related={related}
              usage={settings.usageScan ? usage : undefined}
            />
          </div>
          {inspected && (
            <Inspector
              plugin={inspected}
              usage={settings.usageScan ? usageFor(inspected, usage) : undefined}
              selected={selected}
              onToggleInstall={toggleInstall}
              onClose={() => setInspectedKey(null)}
            />
          )}
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

        {settingsOpen && (
          <SettingsModal
            settings={settings}
            onChange={changeSettings}
            onClose={() => {
              setSettingsOpen(false);
              const dirs = getSettings().extraScanDirs;
              if (JSON.stringify(dirs) !== JSON.stringify(scannedDirs.current)) rescan();
            }}
          />
        )}

        {exportChoice && (
          <ExportModal
            count={plugins.length}
            selectedCount={selected.size}
            onChoose={(w) =>
              doExport(
                w === "selected" ? sortPlugins(mergePlugins(selectedBundles), sort.key, sort.dir, usage) : plugins,
              )
            }
            onCancel={() => setExportChoice(false)}
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

        {exportedDir && !results && (
          <div className={`toast${exportedDir === "error" ? " toast-error" : ""}`} role="status">
            {exportedDir === "error" ? (
              <strong>Export failed</strong>
            ) : (
              <>
                <strong>Inventory exported to Downloads</strong>{" "}
                <button className="ghost small" onClick={() => revealInFinder(exportedDir)}>
                  Reveal
                </button>
              </>
            )}
          </div>
        )}
      </main>
    </div>
  );
}
