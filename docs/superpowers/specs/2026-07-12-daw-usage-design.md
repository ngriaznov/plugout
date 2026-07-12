# DAW usage cross-reference

Approved 2026-07-12. Answers "is this plugin actually used?" by scanning DAW
project files for plugin references and showing per-plugin usage in the list
and Inspector. Merged to main when done; version roll deferred until the user
has tried it.

## Recon ground truth (this machine)

- REAPER `.RPP`: plain text; plugin lines look like
  `<AU "AU: Ozone 12 Unlimiter (iZotope)" "iZotope: Ozone 12 Unlimiter" ...`
  and `<VST "VST3: Ozone 12 Vintage Limiter (iZotope)" "Ozone 12 Vintage Limiter.vst3" ...`
  — first quoted string is `FORMAT: Name (Vendor)`.
- Ableton `.als`: gzip-compressed XML. Third-party devices appear as
  `<VstPluginInfo>`/`<Vst3PluginInfo>`/`<AuPluginInfo>` blocks containing
  `<PlugName Value="..."/>` (VST2) or `<Name Value="..."/>` plus
  `<Manufacturer Value="..."/>` (AU/VST3). Native Live devices have no such
  blocks and are correctly invisible to us.
- No desktop Logic on this machine; Logic's binary project format is out of
  scope for now.

## Backend (Rust)

New module `src-tauri/src/usage.rs` + one command.

### Discovery

- Primary: shell out to `mdfind` twice (`kMDItemFSName == "*.rpp"c`,
  `kMDItemFSName == "*.als"c`). Case-insensitive match covers `.RPP`.
- Fallback (mdfind missing/error/empty): recursive walk of `~/Documents` and
  `~/Music`, max depth 8, collecting the same extensions.
- Skip noise: any path containing a `/Backup/` segment (Ableton auto-backups)
  and REAPER `.rpp-bak` files. Skip files > 64 MB (pathological).
- The shell-out sits behind a trait seam (`ProjectFinder`) like `PkgUtil`
  does, so parsing is tested without Spotlight.

### Parsing

- `parse_rpp(text: &str) -> Vec<PluginRef>`: regex over
  `<(VST|AU|CLAP|LV2)i? "..."` lines; from the first quoted string strip the
  leading `FORMAT: ` prefix and split a trailing ` (Vendor)`. JS (built-in
  REAPER effects) lines are ignored.
- `parse_als(bytes: &[u8]) -> Vec<PluginRef>`: gzip-decode (new dependency
  `flate2 = "1"`), then regex the XML text for plugin-info blocks:
  `<PlugName Value="..."/>` or `<Name Value="..."/>` within ~500 chars after a
  `<VstPluginInfo|Vst3PluginInfo|AuPluginInfo|ClapPluginInfo` opener, plus the
  nearest `<Manufacturer Value="..."/>` when present (vendor may be empty for
  VST2). No XML parser dependency — the two value patterns are stable.
- `PluginRef { name: String, vendor: String }` (vendor may be `""`).
- Refs are deduplicated per project file.

### Command

`scan_usage() -> Vec<UsageHit>` where
`UsageHit { name, vendor, project: String (path), mtimeMs: f64 }` — one row
per (project, deduped ref). Async command on the blocking pool (IO). Serde:
camelCase field `mtimeMs` via `#[serde(rename_all = "camelCase")]`; other
fields single-word.

## Frontend

### Matching (`src/util.ts`)

Export existing private helpers needed: `tokensOf`, `digitsOf` stay private;
add exported `matchUsage(plugins: Plugin[], hits: UsageHit[]): Map<string, Usage>`
keyed by `Plugin.key`, where
`Usage { projects: number; lastUsedMs: number; lastProject: string }`.

Matching per hit, against each plugin's installs:
1. Exact: `fold(hit.name) === fold(install.name)` and, when `hit.vendor` is
   non-empty, vendor folds match by containment either way ("iZotope" vs
   "izotopeinc"). Empty hit vendor ⇒ name match alone suffices.
2. Fallback: equal digit sequences AND one name's token set ⊆ the other's
   (reuses the family-absorption predicates on the two names) AND the vendor
   condition above.
`projects` counts distinct project paths; `lastUsedMs`/`lastProject` from the
max mtime. Plugins with no hits are absent from the map.

### UI

- App.tsx: after a scan settles (same effect timing as search indexing), call
  `scanUsage()` (api wrapper + mock returning `[]`), store hits; memo builds
  the usage map from the CURRENT merged plugins.
- PluginList: new sortable column "Used" between Version and Size — shows the
  project count, or "—" when unseen. Sort key `used`: by `lastUsedMs` desc,
  unseen last (both directions keep unseen last).
- Inspector: under the header sub-line, when usage exists:
  `Used in N project(s) · last <YYYY-MM-DD>` with a Reveal button for the last
  project file. When no usage: `Not seen in any DAW project` in muted text —
  the honest delete-candidate signal, with a title tooltip noting only
  REAPER/Ableton projects are scanned.
- Column and any usage-driven affordances must not imply certainty: tooltip on
  the column header states the scanned sources.

## Failure handling

Everything degrades to "no usage data": mdfind failing, unreadable/corrupt
project files (skipped per file, never abort the batch), no projects found.
No error UI; a `console.warn` for the command-level failure.

## Testing

- Rust: parser tests with fixture strings for both formats (real-world shapes
  from the recon above, including a JS line to ignore, a vendor-less VST2
  block, and a corrupt gzip that must yield an empty vec, not an error).
  Finder trait spy test: fallback walk used when mdfind errors.
- TS: `matchUsage` — exact fold match, containment vendor, family fallback
  ("Ozone 12 Vintage Limiter" hit vs merged product), dedup of project counts,
  unseen plugins absent.
- Component: PluginList Used column renders count and dash; sort places
  unseen last both directions. Inspector shows both usage states.

## Out of scope

Logic/Cubase/FL project formats; watching projects for changes; per-track
detail; config UI for scan locations.
