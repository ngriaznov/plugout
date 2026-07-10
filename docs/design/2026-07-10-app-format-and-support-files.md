# APP as a format + receipt-based support-file cleanup

**Status:** Approved 2026-07-10

Completes the "receipt-based reversal" the README scope note promised: companion
applications become a first-class `APP` format chip, and installer-owned support
files are offered — guarded — as part of a plugin's removal.

## APP format

- `Format::App` (serialized `"APP"`), rendered as a sixth chip. An app entry is a
  normal `PluginBundle`: path `/Applications/X.app` (or `~/Applications`),
  name/version from the app's `Info.plist`, size, scope, `packageId`.
- Merging by (vendor, name) puts `Serum.app` on the Serum row (`AU VST3 VST2 APP`).
  Vendor tools (e.g. "Arturia Software Center") don't name-match a plugin and form
  their own APP-only row — visible and removable, never dragged along.
- Discovery, deduped by path, streamed via the existing `scan:batch` event:
  1. **Receipts:** for each unique `packageId`, `pkgutil --files` entries matching
     `Applications/*.app` (resolved against the package volume + install-location).
  2. **Name match:** `/Applications/<PluginName>.app` and the `~/Applications`
     equivalent, for receipt-less plugins.

## Support files

Attached to a plugin row's removal, not a chip. Shown in the inspector's
"Will be moved to Trash" section and as a per-plugin line in the confirm modal
("+ N items · X MB", expandable) with a toggle.

Guards, all receipt-driven — no name heuristics for files:

1. **Exclusivity:** a package's support files are offered only when every plugin
   bundle owned by that package is included in the removal. Toggle defaults on
   when exclusive; when a package is shared with plugins staying installed, its
   files are skipped and the modal says "some files kept: shared with N plugins".
2. **Directory roll-up with subtree proof:** offer a directory only when its
   entire on-disk contents are owned by packages in the removal; otherwise offer
   the individually-owned files only.
3. **Allowlisted roots:** candidates must live under `Library/Application Support`,
   `Library/Preferences`, `Library/Caches`, `Library/Audio` (non-plugin subdirs),
   or `/Users/Shared` — under either Library root. Receipt entries outside the
   allowlist are never offered.
4. Everything goes to the Trash under the existing scope-batched flow (one admin
   prompt), so mistakes stay recoverable.

## Contracts

- `removal_preview(ids) -> { supportFiles: [{path, sizeBytes}], skippedShared: u32 }`
  — new command the confirm modal calls when it opens; pure logic behind the
  `PkgUtil` trait (new `files(pkg)` and `pkg_info(pkg)` methods on it).
- `remove_items` is unchanged: the modal appends the toggled-on support paths to
  the bundle ids.
- Frontend: `Format` union gains `"APP"` (chip color rose), sidebar count comes
  free, ConfirmModal gains the async preview + toggle, Inspector renders an
  "Application" card for APP installs. Mock backend grows an APP example, a
  vendor-tool row, and support-file previews so the browser dev mode exercises
  all of it.

## Testing

Rust: path resolution from pkgutil output, app discovery (receipt + name-match +
dedup), exclusivity, roll-up subtree proof, allowlist filtering — all pure units
on mocked `PkgUtil`/filesystem fixtures. Frontend: merge with APP chips, modal
preview states (exclusive / shared / empty), toggle wiring.

## Out of scope

Running-app detection before trashing an APP (Trash is recoverable; may follow),
Windows, and any non-receipt heuristics for files.
