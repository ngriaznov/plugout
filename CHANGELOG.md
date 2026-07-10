# Changelog

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
