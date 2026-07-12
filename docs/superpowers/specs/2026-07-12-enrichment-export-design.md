# Search corpus enrichment + inventory export

Approved 2026-07-12. Two features, released together as 0.2.5.

## 1. Corpus enrichment (search v2)

Semantic search under-delivers because plugin names are brand words. Fix: append
curated keywords to each document's embed text, backend-side.

### Data & matching (`src-tauri/src/keywords.rs`, new)

- `fn fold(s: &str) -> String` — lowercase + strip non-alphanumerics (mirrors
  the frontend's grouping fold).
- Two static tables:
  - `VENDOR_KEYWORDS: &[(&str, &str)]` — folded-vendor *substring* key →
    keywords. Keys are distinctive substrings so vendor spelling variants match
    ("d16group" hits both "d16group" and "d16groupaudiosoftware").
  - `NAME_KEYWORDS: &[(&str, &str)]` — folded-name substring key → keywords.
    Keys must be ≥4 chars to avoid accidental hits.
- `pub fn enrich(name: &str, vendor: &str) -> String` — concatenated keywords
  from every matching entry (vendor + name matches are additive), deduplicated
  by word, space-joined; empty string when nothing matches.
- Applied in `index_search` (commands.rs): doc text becomes
  `format!("{} {}", d.text, keywords)` when keywords are non-empty.

### Initial curation (implementer refines during verification)

Vendor entries (folded key → keywords), weighted toward the real library:
arturia→"analog synthesizer vintage keys emulation"; valhalla→"reverb delay
echo"; fabfilter→"equalizer compressor limiter filter mixing"; talsoftware→
"analog synthesizer effect"; d16group→"drum machine bass synthesizer";
xfer→"wavetable synthesizer"; nativeinstruments→"sampler synthesizer drums";
izotope→"mastering mixing equalizer restoration"; ssl→"channel strip console
equalizer compressor mixing"; solidstatelogic→same as ssl; moog→"analog
synthesizer bass ladder filter"; korg→"synthesizer keys"; roland→"synthesizer
drum machine vintage"; noiseengineering→"modular oscillator synthesizer
percussion"; discodsp→"synthesizer sampler"; mok→"wavetable synthesizer";
appliedacoustics→"physical modeling synthesizer"; tracktion→"synthesizer";
madronalabs→"synthesizer additive modular"; vcvrack→"modular synthesizer
eurorack"; newfangledaudio→"synthesizer saturation limiter"; eventide→
"harmonizer pitch delay reverb"; harrisonaudio→"channel strip console
equalizer mixing"; unfilteredaudio→"delay glitch effect"; uhe→"analog
wavetable synthesizer"; soundtoys→"saturation delay effect"; spectrasonics→
"synthesizer sampler"; sonnox→"equalizer compressor mastering"; apple→
"spatial audio renderer system".

Name entries: proq→"equalizer"; prol→"limiter"; proc→"compressor"; pror→
"reverb"; vintageverb→"reverb"; supermassive→"reverb"; drumazon→"drum machine
909"; nepheton→"drum machine 808"; decimort→"bit crusher sampler"; kontakt→
"sampler"; battery→"drums sampler"; serum→"wavetable synthesizer";
retromulator→"vintage sampler"; waverazor→"wavetable synthesizer"; miniraze→
"wavetable synthesizer"; mariana→"analog bass synthesizer".

### Verification (mandatory, not optional)

Rerun the real-corpus replica (scratchpad `ternlight/engine` example + the
`corpus.tsv` built from the machine's actual plugins, WITH enrichment applied
the same way) and require, versus the 2026-07-11 baseline:

- "equalizer" surfaces SSL/Harrison channel strips (baseline: none/junk).
- "reverb" ranks Eventide/TAL/Valhalla-class entries above Retromulator
  (baseline: Retromulator false positive).
- "piano", "drum machine", "synth", "moog" stay at least as good.

Tune table entries until these hold; record before/after in the report.

### Non-goals

No per-user editable keyword file, no remote data, no UI.

## 2. Inventory export

### Pure builders (`src/export.ts`, new)

- `exportCsv(plugins: Plugin[]): string` — one row per install, columns:
  `product,name,vendor,version,format,scope,category,sizeBytes,path,bundleId,packageId`.
  `product` is the merged Plugin's display name; `name`/`vendor` are the
  install bundle's own. RFC-4180 quoting (quote fields containing `",\n`,
  double embedded quotes). Header row always present. Rows follow the
  incoming plugin order; installs in FORMATS order (as merged).
- `exportJson(plugins: Plugin[]): string` — pretty-printed array of products:
  `{ name, vendor, version, sizeBytes, category, installs: [{ name, vendor,
  version, format, scope, category, sizeBytes, path, bundleId, packageId }] }`.
- Unit tests in `src/export.test.ts` (quoting, header, nesting, empty list).

### Backend (`commands.rs`)

`save_export(app: AppHandle, files: Vec<ExportFile>) -> Result<String, String>`
where `ExportFile { name: String, contents: String }`. Resolves the user's
Downloads directory via Tauri's path resolver, writes each file, returns the
directory path (for Reveal). Rejects file names containing path separators.

### Frontend wiring

- `api.ts`: `saveExport(files): Promise<string>` (+ mock returning "/tmp").
- App.tsx toolbar: ghost "Export" button next to Rescan, disabled while
  `loading` or when `bundles` is empty. On click: build
  `plugout-inventory-YYYY-MM-DD.csv` and `.json` from the FULL merged list
  (unfiltered `mergePlugins(bundles)`, name-sorted), call `saveExport`, then
  toast "Inventory exported to Downloads" with a Reveal button that calls
  `revealInFinder(returnedDir)`. Errors: toast "Export failed" (reuse the
  existing toast pattern; no new UI system).

## Process

- Sonnet 5 subagents per task (established preference), TDD, per-task review.
- Before the release commits: run the oss-polishing sweep (user instruction)
  and fold its actionable findings in.
- Release: CHANGELOG 0.2.5 entry, version roll per repo pattern, merge to
  main, verify CI green and the Release workflow's tag/artifacts.
