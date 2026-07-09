# plugout

A macOS app to list and remove audio plugins — AU, VST3, VST2, CLAP, AAX. Removal is
**recoverable**: plugins are moved to the Trash, never permanently deleted.

## Features
- Scans user (`~/Library`) and system (`/Library`) plugin folders across all five formats.
- Filter by format and location, live search, multi-select, and a running total of
  reclaimable space.
- **One row per plugin** — a plugin's AU/VST3/VST2/CLAP/AAX installs are merged into a
  single row with per-format chips; select the whole plugin or individual formats.
- Inspector panel shows each install's version, size, location, bundle id, which macOS
  installer package placed it, and the exact files that will be moved to the Trash.
- **Removal moves the selected plugin bundle(s) to the Trash** (recoverable). User-scope
  plugins need no privileges; system-scope removals prompt once for an administrator
  password (batched — one prompt per removal, not one per plugin).
- Reveal any plugin in Finder.

## Scope note
This POC removes the **plugin bundle itself** (the `.vst3` / `.component` / `.vst` /
`.clap` / `.aaxplugin`). Full receipt-based reversal — also trashing the extra support
files an installer wrote elsewhere, while protecting files shared between plugins — is a
planned follow-up: doing it safely requires resolving `pkgutil`'s install-location-relative
paths and guarding against ever removing a shared directory. Bundle removal is the safe,
self-contained core.

## Develop
```bash
npm install
npm run tauri dev
```

`npm run dev` alone serves the frontend in a browser against a mock backend
(`src/api.mock.ts`) — handy for UI work without a Rust build.

## Test
```bash
cd src-tauri && cargo test      # backend (15 tests)
npm test                        # frontend (formatBytes + plugin merging)
```

## Build
```bash
npm run tauri build
```

> macOS only for now. Windows/Linux later.
