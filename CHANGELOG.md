# Changelog

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
