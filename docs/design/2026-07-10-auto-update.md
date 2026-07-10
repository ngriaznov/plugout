# Auto-update from GitHub releases

**Status:** Approved 2026-07-10

plugout updates itself from GitHub releases using Tauri's updater plugin, presented
as a non-blocking pill in the toolbar.

## Trust chain

- A minisign keypair signs updates. The public key is embedded in
  `tauri.conf.json` (`plugins.updater.pubkey`); the private key lives only in the
  `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret (and the maintainer's
  `~/.tauri/plugout.key`).
- The app only installs updates whose signature verifies against the embedded key.
- Losing the private key means shipping one manual-download release with a new
  public key.

## Release pipeline

`bundle.createUpdaterArtifacts: true` makes the universal build emit
`plugout.app.tar.gz` + `plugout.app.tar.gz.sig` next to the DMG. The release
workflow additionally:

1. exports `TAURI_SIGNING_PRIVATE_KEY` (+ empty `_PASSWORD`) during the build;
2. writes `latest.json` — version, `pub_date`, and the `.tar.gz` URL + signature
   under both `darwin-aarch64` and `darwin-x86_64` (one universal artifact serves
   both);
3. uploads DMG, `.app.tar.gz`, and `latest.json` as release assets.

The app checks the stable URL
`https://github.com/ngriaznov/plugout/releases/latest/download/latest.json`,
so publishing a release is the entire rollout.

## App side

- `tauri-plugin-updater` + `tauri-plugin-process` registered in `lib.rs`
  (desktop only); capabilities grant `updater:default` and `process:allow-restart`.
- `src/updater.ts` wraps check/download/install behind the same
  outside-Tauri guard as `api.ts` (browser dev mode reports no update).
- State machine in App: `idle → available → downloading(progress) → ready | error`.

## UX

Silent check shortly after launch. If a newer version exists, a pill appears in
the toolbar next to Rescan: "v0.2.0 available" → click → inline download
progress → "Restart to update" → relaunch. Failures surface in the existing
toast style; there is no blocking dialog and no nagging — the pill just sits
there until used.

`UpdatePill` is a presentational component driven by a state prop, unit-tested
like the other components.

## Rollout

The first updater-capable release cannot itself arrive by auto-update; v0.1.1
users download once more manually. Verification: release 0.1.2, install it,
release 0.1.3, watch the installed app update itself in place.
