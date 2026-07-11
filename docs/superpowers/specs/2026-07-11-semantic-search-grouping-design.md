# Semantic search, smarter grouping, per-install selection

Design for three changes to plugout, approved 2026-07-11:

1. Semantic ("related") search powered by ternlight, embedded in the Rust backend.
2. Grouping fixes so spelling/family variants merge into one plugin row.
3. Per-install checkboxes in the Inspector so users can remove one format and keep others.

## 1. Semantic search (Rust backend)

### Engine

ternlight (github.com/soycaporal/ternlight) publishes no Rust crate; its `engine/`
crate (`tern-engine`) compiles natively (verified: scalar kernel path exists behind
`cfg(not(target_arch = "wasm32"))`, release build + inference work on macOS).

- Vendor the crate at `src-tauri/vendor/tern-engine` (copied from the repo at
  pinned commit `f67b04f99e7ee83782e3ad3c81bf4f3f0e0d0bd8`; keep its LICENSE).
  A plain Cargo git dependency was ruled out: the crate has no `build.rs` and
  `model.rs` unconditionally `include_bytes!`es `assets/model.bin`, which is not
  committed upstream — the dep would fail to compile as checked out.
- The model binary is NOT in the repo — it is a release artifact. Add a `build.rs`
  to the vendored crate that downloads `model-embedding-int8.bin` (8.7 MB) from
  `huggingface.co/wenshutang/ternlight` into `OUT_DIR`, verifies a pinned SHA-256,
  and change `model.rs` to `include_bytes!(concat!(env!("OUT_DIR"), "/model.bin"))`.
  Build offline after first fetch (cache the download under `OUT_DIR`; skip when
  present and hash matches).
- Feature: `emb_int8` (the "base" quality tier).

### Tauri commands (commands.rs)

- `index_search(docs: Vec<SearchDoc>)` — `SearchDoc { id: String, text: String }`.
  Embeds each text (384-dim, L2-normalized) and stores `Vec<(String, Vec<f32>)>`
  in managed state (`Mutex`), replacing any previous index. `async` command so
  indexing never blocks the UI.
- `semantic_search(query: String) -> Vec<SearchHit>` — `SearchHit { id, score }`.
  Embeds the query, scores by dot product (vectors are normalized), returns hits
  with `score >= 0.30`, sorted desc, capped at 8. Constants live in one place in
  Rust; 0.30 is calibrated from measurement (query→name scores run low: "reverb"
  vs a reverb plugin ≈ 0.23–0.39).

### Frontend

- `api.ts`: `indexSearch(docs)`, `semanticSearch(query)`; `api.mock.ts` returns
  resolved no-op / `[]` so browser dev keeps working.
- After a scan delivers bundles, App builds docs — one per bundle:
  `"{name} {vendor} {category label}"` — and fires `indexSearch` (not awaited;
  errors logged and ignored).
- Query flow: when `query.length >= 3`, debounce ~150 ms, call `semanticSearch`.
  Result ids that are NOT already substring matches become the "related" set.
- UI: PluginList shows substring matches as today; below them, a divider
  ("Related") followed by the merged related plugins. Empty related set → no
  divider. Any backend error → substring-only behavior, no error UI.

## 2. Grouping fixes (src/util.ts, `mergePlugins`)

Two additive rules. Ground truth that motivates them:

- sumu: AU vendor "Madrona Labs" vs VST3 fallback vendor "madronalabs";
  bundleIds differ (`com.madronalabs.vst3plugin.sumu.audiounit` vs
  `com.madronalabs.vst3.sumu`) → today, no key bridges the installs.
- VCV Rack 2: one AU component registers "VCV Rack 2", "VCV Rack 2 MIDI FX",
  "VCV Rack 2 FX" (same bundleId `com.vcvrack.rack`); plus VST3/CLAP
  "VCV Rack 2" and the standalone app "VCV Rack 2 Pro.app". All are one product.
- Serum / Serum FX are one product; Serum vs Serum 2 (and Pro-Q 3 vs Pro-Q 4,
  Kontakt 7 vs 8) are different products — digit changes mean a different product.

### Rule A — aggressive key normalization

`fold(s)` becomes: lowercase + strip every non-alphanumeric character.
Applied to both vendor and name in merge keys (use an explicit separator, e.g.
`vn:{vendor}|{name}`, since stripping removes spaces). Effects:

- "Madrona Labs" ≡ "madronalabs" (fixes sumu).
- "Serum 2" ≡ "Serum2", "TAL-Reverb-4" ≡ "TAL Reverb 4" (spelling variants).

### Rule B — family absorption pass

After the existing union-find groups form, run a second pass over groups:

- `tokens(name)`: lowercase alphanumeric tokens, minus format tokens
  {vst, vst2, vst3, au, audiounit, aax, clap, app}.
- `digits(name)`: the ordered sequence of numeric tokens (post format-token
  removal, so "(VST3)" never contributes a digit).
- Group identity set: all folded vendors ∪ all bundleIds ∪ all reverse-DNS org
  prefixes (first two bundleId segments, e.g. `com.vcvrack`) of its members.
- Merge group B into group A when: identity sets intersect, AND
  `digits(A) == digits(B)`, AND `tokens(A) ⊊ tokens(B)` (A's name is the base).
- Absorption is transitive via union-find (FX and MIDI FX both land in the
  Rack 2 family even though they also subset each other).
- Display name: the member name with the fewest tokens (tie → shortest string).

Must-hold cases (all become tests):

| Case | Expected |
| --- | --- |
| sumu AU + Sumu VST3 (vendor spellings) | merge |
| VCV Rack 2 + MIDI FX + FX + Pro (app) | one group, named "VCV Rack 2" |
| Serum + Serum FX | merge |
| Serum vs Serum 2 / Serum FX vs Serum 2 FX | stay separate (digits differ) |
| Pro-Q 3 vs Pro-Q 4, Kontakt 7 vs Kontakt 8 | stay separate |
| Ozone 11 Equalizer vs Ozone 11 Imager | stay separate (neither token set ⊂ other) |
| Pro-Q 3 vs FabFilter Pro-Q 3 | merge (vendor FabFilter, subset, digits equal) |

### Shared-path bookkeeping

Multiple AU registry entries share one `.component` file, so a family group can
contain several installs with the same `path`. Therefore:

- `sizeBytes` of a Plugin sums over unique paths only.
- Removal dedupes selected bundles by path before invoking the backend.

## 3. Per-install selection (Inspector)

Selection is already `Set<bundle id>` in App.tsx — no data-model change.

- Each `InstallCard` gets a checkbox bound to that set (`checked`, `onToggle`
  props threaded through `Inspector`).
- The plugin row checkbox in `PluginList` becomes tri-state: checked when all
  installs selected, indeterminate when some are.
- ActionBar and the removal flow are unchanged (they already operate on the
  selected-bundle set), except for path dedup above.

## Testing

- `util.test.ts`: every row of the must-hold table; display-name and
  unique-path-size assertions.
- Rust: unit test for top-K/threshold logic with stub vectors; `index_search`
  then `semantic_search` round-trip behind `cargo test` (embeds real model).
- Component tests: PluginList tri-state checkbox; Inspector checkbox toggling.
- Manual: run app, verify sumu/VCV rows merge, "reverb" surfaces related
  plugins, deleting AAX-only leaves VST3 install intact.

## Out of scope

- Richer corpus text (descriptions/tags) — name+vendor+category only for now.
- Persisting the embedding index across launches (rebuilt per scan; it's fast).
- mini/ternary model tiers.
