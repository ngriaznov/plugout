import { useEffect, useState } from "react";
import type { Plugin, PluginBundle, PluginDetails } from "../types";
import { CATEGORY_LABELS } from "../types";
import { revealInFinder } from "../api";
import { getDetails } from "../detailsCache";
import { FormatBadge } from "./FormatBadge";
import { formatBytes, type Usage } from "../util";

type DetailState = PluginDetails | "error" | undefined;

function InstallCard({
  bundle,
  details,
  checked,
  onToggle,
}: {
  bundle: PluginBundle;
  details: DetailState;
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <section className="install-card">
      <header className="install-head">
        <input
          type="checkbox"
          aria-label={`Select ${bundle.scope} ${bundle.format} install`}
          checked={checked}
          onChange={onToggle}
        />
        <FormatBadge format={bundle.format} />
        <span className="install-version">v{bundle.version || "?"}</span>
        <span className="install-size">{formatBytes(bundle.sizeBytes)}</span>
      </header>

      <dl className="kv">
        {bundle.format === "APP" && (
          <div><dt>Kind</dt><dd>Companion application</dd></div>
        )}
        <div><dt>Location</dt><dd>{bundle.scope === "user" ? "User" : "System"}</dd></div>
        <div>
          <dt>Installed by</dt>
          <dd>
            {details === undefined && <span className="skel skel-inline" />}
            {details === "error" && <span className="error-note">couldn’t load details</span>}
            {details !== undefined && details !== "error" && (
              <code>{details.packageId ?? "not from an installer"}</code>
            )}
          </dd>
        </div>
        {bundle.bundleId && (
          <div><dt>Bundle ID</dt><dd><code>{bundle.bundleId}</code></dd></div>
        )}
      </dl>

      <div className="path-row">
        <code className="path">{bundle.path}</code>
        <button className="ghost small" onClick={() => revealInFinder(bundle.path)}>
          Reveal
        </button>
      </div>

      {details === undefined && (
        <div className="files-skel" aria-hidden="true">
          <div className="skel" style={{ width: "82%" }} />
          <div className="skel" style={{ width: "58%" }} />
        </div>
      )}
      {details !== undefined && details !== "error" && details.filesToTrash.length > 1 && (
        <>
          <div className="group-label">Will be moved to Trash</div>
          <ul className="filelist">
            {details.filesToTrash.map((f) => <li key={f}>{f}</li>)}
          </ul>
        </>
      )}
    </section>
  );
}

export function Inspector({
  plugin,
  usage,
  selected,
  onToggleInstall,
  onClose,
}: {
  plugin: Plugin;
  usage?: Usage | null;
  selected: Set<string>;
  onToggleInstall: (id: string) => void;
  onClose: () => void;
}) {
  const [details, setDetails] = useState<Record<string, DetailState>>({});

  useEffect(() => {
    let active = true;
    setDetails({});
    for (const b of plugin.installs) {
      getDetails(b.id).then(
        (d) => active && setDetails((m) => ({ ...m, [b.id]: d })),
        () => active && setDetails((m) => ({ ...m, [b.id]: "error" })),
      );
    }
    return () => {
      active = false;
    };
    // installs are keyed by the plugin identity; refetch when it changes
  }, [plugin.key]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <aside className="inspector">
      <header className="inspector-head">
        <div>
          <div className="inspector-titlerow">
            <h2 className="inspector-title">{plugin.name}</h2>
            {plugin.category && (
              <span className="category-chip">{CATEGORY_LABELS[plugin.category]}</span>
            )}
          </div>
          <div className="inspector-sub">
            {plugin.vendor}
            {plugin.version && <> · v{plugin.version}</>} · {formatBytes(plugin.sizeBytes)}
          </div>
          {plugin.copyright && <div className="inspector-copyright">{plugin.copyright}</div>}
          {usage !== undefined && (
            <div className="inspector-usage" title="From REAPER (.rpp) and Ableton (.als) project files found on this Mac">
              {usage ? (
                <>
                  Used in {usage.projects} project{usage.projects === 1 ? "" : "s"}
                  {usage.lastUsedMs > 0 && (
                    <> · last {new Date(usage.lastUsedMs).toISOString().slice(0, 10)}</>
                  )}{" "}
                  <button className="ghost small" onClick={() => revealInFinder(usage.lastProject)}>
                    Reveal
                  </button>
                </>
              ) : (
                <span className="usage-none">Not seen in any DAW project</span>
              )}
            </div>
          )}
        </div>
        <button className="x" aria-label="Close details" onClick={onClose}>✕</button>
      </header>

      {plugin.installs.map((b) => (
        <InstallCard
          key={b.id}
          bundle={b}
          details={details[b.id]}
          checked={selected.has(b.id)}
          onToggle={() => onToggleInstall(b.id)}
        />
      ))}
    </aside>
  );
}
