# Changelog

## 0.4.3

- The sidebar shows a disk-usage chart below the filters: one stacked bar of
  your plugins' total size by format, with per-format sizes listed beneath.
  Colors are colorblind-checked in both themes.
- Settings reorganized into three labeled sections in a more natural order —
  Scan locations, Plugin usage, Appearance — with consistent dividers.

## 0.4.2

- The theme switcher (Light / Auto / Dark) moved from the sidebar into
  Settings, under a new Appearance section.
- (0.4.1 was withdrawn shortly after publishing and never reached general
  availability; this release supersedes it.)

## 0.4.0

- Rescans link installer receipts almost instantly: receipts are now cached to
  disk, keyed by each bundle's path and modification time, so only new or
  changed bundles re-run `pkgutil`.
- The Used column and inspector now also read Studio One (`.song`) and Logic
  Pro (`.logicx`) projects, alongside the existing REAPER and Ableton support.
  Logic project data isn't a documented format, so matches there are
  best-effort name lookups rather than an exact reference count.
- New Settings → Scan locations: add extra folders to scan for plugins via a
  native folder picker; closing Settings rescans automatically if the list
  changed.
- Full keyboard navigation: `/` or Cmd+F jumps to search, arrow keys move
  between rows (Home/End to jump to the first/last), Space selects a row,
  Enter opens its inspector, and dialogs trap focus and hand it back to
  where you were when closed.
- Export now follows whatever the table is currently showing (search,
  filters). With a selection active, Export asks whether to write the
  selected plugins or everything shown.
- Canceling the admin password prompt during a system-scope removal no
  longer shows an error — it just keeps your selection so you can try again.
- Dates in the UI and export filenames use your local timezone instead of UTC.
- Internal: added a Playwright end-to-end suite (`npm run e2e`, 10 specs)
  running against the mock-backend dev server, wired into CI; the Vitest
  suite grew to 141 tests, including App-level integration coverage.

## 0.3.0

- The window now uses a seamless overlay title bar: the app's background fills
  the whole window, title bar included, so it always matches the current
  light/dark theme instead of showing macOS's default title bar. The top of
  the window stays draggable.

## 0.2.9

- Internal: migrated the Rust backend to the 2024 edition and applied
  idiomatic cleanup (let-chains, formatting). No user-facing changes.

## 0.2.8

- Fixed the plugin table's header row staying sharp above the blur when the
  settings or removal dialog was open.

## 0.2.7

- Added a Settings dialog. DAW project scanning is now opt-in and off by
  default, since it can trigger macOS folder-access prompts — enable it in
  Settings.

## 0.2.6

- New Used column shows, per plugin, how many REAPER and Ableton projects
  reference it and when it was last used, sortable to find safe delete
  candidates; the inspector shows the same usage line for the selected plugin.
- Project usage is found by scanning Spotlight-located .rpp/.als files for
  their plugin reference blocks, read-only and skipped silently when a
  project file can't be parsed.
- Related search now also reads each VST3 bundle's own moduleinfo.json
  subcategories (EQ, Dynamics, Reverb…), so queries like "equalizer" and
  "compressor" match plugins by what they are, not just by name.
- Fixed the plugin table collapsing (blank names, clipped format chips) when
  the Related section was visible on narrow windows.

## 0.2.5

- Related search now understands function words: curated per-vendor keywords
  are folded into each plugin's search document, so queries like "reverb" or
  "equalizer" surface relevant gear even when the plugin's name never says it.
- One-click inventory export: the Export button writes a CSV and JSON
  snapshot of every product, install, version, path and installer package to
  Downloads, with a Reveal action in the confirmation toast.

## 0.2.4

- Search now shows semantically related plugins in a "Related matches" section
  below the direct substring matches, for queries of three characters or more.
- Plugins whose formats spell the vendor or name differently (e.g. "sumu" AU
  vs "Sumu" VST3, or a synth's AU/VST3/CLAP installs plus its companion app)
  now merge into a single row instead of appearing as duplicates.
- The inspector's install cards each get their own checkbox, so a single
  format can be selected for removal without selecting the whole plugin.

## 0.2.3

- The inspector header now shows the plugin's category (Instrument / Effect /
  MIDI Effect, from the AU component type) and copyright, read from the bundle.

## 0.2.2

- Companion apps are found by walking the Applications folders instead of
  deriving them from installer receipts — they now appear with the scan instead
  of a minute later. Apple system apps can never match.
- A "linking installers…" indicator shows while receipt enrichment runs.

## 0.2.0

- Companion applications appear as an `APP` format chip — on the plugin's row when
  names match, on their own row for vendor tools. Linked by installer receipt or name.
- Removal now offers the installer's support files (presets, preferences, caches),
  guarded: receipt evidence only, skipped when the installer is shared with plugins
  staying installed, allowlisted Library roots only, toggleable in the confirmation.

## 0.1.4

- One plugin, one row: installs whose AU vendor name differs from the
  bundle-id vendor of their VST/AAX siblings (e.g. "discoDSP" vs "discodsp",
  "D16 Group Audio Software" vs "d16group") no longer show as duplicates.

## 0.1.3

- First release delivered through the built-in updater.

## 0.1.2

- Auto-update from GitHub releases: signed updates, checked quietly at launch,
  installed from a toolbar pill with one click and a restart.

## 0.1.1

- New app icon: an O sliced through by a diagonal line.
- Internal cleanup: removal API takes bundle paths directly, crate-level docs.

## 0.1.0

Initial release.

- Scan AU, VST3, VST2, CLAP and AAX plugins in user and system folders, streamed live.
- One row per plugin with per-format selection via chips.
- Inspector with per-install version, size, scope, installer receipt and files-to-trash.
- Sortable columns: name, vendor, formats, version, size.
- Removal to Trash; single admin prompt for system-scope batches.
- Installer receipt linking via `pkgutil` in the background.
- Light/dark/auto theme.
