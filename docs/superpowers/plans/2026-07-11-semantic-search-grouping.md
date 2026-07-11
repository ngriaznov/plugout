# Semantic Search, Family Grouping, Per-Install Selection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge spelling/family variants of the same plugin into one row, add a semantic "Related" section to search (ternlight engine embedded in the Rust backend), and let users select individual installs from the Inspector.

**Architecture:** Grouping is pure TypeScript in `src/util.ts` (`mergePlugins`): an aggressive key fold plus a post-pass that absorbs "base name + suffix words" groups via union-find. Semantic search vendors ternlight's `tern-engine` Rust crate into `src-tauri/vendor/`, downloads the pinned model in `build.rs`, and exposes two Tauri commands (`index_search`, `semantic_search`); the React app indexes bundles after each scan and shows sub-threshold-free "Related" rows under the substring matches. Inspector checkboxes bind to the existing `Set<bundle id>` selection.

**Tech Stack:** React 18 + TypeScript + Vite, vitest (+ @testing-library/react, jsdom), Tauri 2 (Rust), vendored `tern-engine` (int8 model).

Spec: `docs/superpowers/specs/2026-07-11-semantic-search-grouping-design.md`

## Global Constraints

- Never add AI/Claude attribution to commits, code, or docs (user's global rule).
- Frontend tests: `npm test` (vitest run) from the repo root — must be green at every commit.
- Rust: `cargo build --release` and `cargo test` run from `src-tauri/`.
- A bundle's `id` IS its filesystem path; the scanner emits exactly one bundle per file, so ids and paths are unique across bundles.
- Semantic search constants (from spec, calibrated by measurement): `MIN_SCORE = 0.30`, `TOP_K = 8`, query min length 3, debounce 150 ms.
- Model: `model-embedding-int8.bin` from `huggingface.co/wenshutang/ternlight`, SHA-256 `5b693903bfc57b1699ca2c3f1d87332801f53a89885867eb63f3f8fc6ccce399`, feature `emb_int8`, upstream commit pin `f67b04f99e7ee83782e3ad3c81bf4f3f0e0d0bd8`.
- The app must degrade gracefully: any search-backend failure ⇒ substring-only search, no error UI.

---

### Task 1: Grouping Rule A — aggressive key normalization

**Files:**
- Modify: `src/util.ts:32-55` (the `fold` + `keysOf` section of `mergePlugins`)
- Test: `src/util.test.ts`

**Interfaces:**
- Consumes: existing `mergePlugins(bundles: PluginBundle[]): Plugin[]`.
- Produces: same signature; `fold(s)` now strips all non-alphanumerics. Task 2 builds on this exact `fold`.

- [ ] **Step 1: Write the failing tests**

Append inside the existing `describe("mergePlugins", ...)` block in `src/util.test.ts` (the `mk` helper already exists at the top of the file):

```ts
  it("merges vendor spelling variants (sumu: 'Madrona Labs' vs 'madronalabs')", () => {
    const plugins = mergePlugins([
      mk({ id: "au", format: "AU", name: "sumu", vendor: "Madrona Labs",
           bundleId: "com.madronalabs.vst3plugin.sumu.audiounit" }),
      mk({ id: "v3", format: "VST3", name: "Sumu", vendor: "madronalabs",
           bundleId: "com.madronalabs.vst3.sumu" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].installs).toHaveLength(2);
  });

  it("merges punctuation and spacing name variants", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", name: "TAL-Reverb-4", vendor: "TAL Software" }),
      mk({ id: "b", format: "VST3", name: "TAL Reverb 4", vendor: "TAL Software" }),
      mk({ id: "c", format: "AU", name: "Serum 2", vendor: "Xfer Records", bundleId: "com.xfer.serum2" }),
      mk({ id: "d", format: "VST3", name: "Serum2", vendor: "Xfer Records", bundleId: "com.xfer.serum2" }),
    ]);
    expect(plugins).toHaveLength(2);
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- util`
Expected: the two new tests FAIL (2 plugins instead of 1 / 4 instead of 2); all pre-existing tests PASS.

- [ ] **Step 3: Implement the fold change**

In `src/util.ts`, inside `mergePlugins`, replace:

```ts
  const fold = (s: string) => s.trim().toLowerCase();
```

with:

```ts
  // Identity fold: case- and punctuation-insensitive, so "Madrona Labs" ≡
  // "madronalabs" and "Serum 2" ≡ "Serum2". Keys need an explicit separator
  // because folding eats the spaces that used to provide one.
  const fold = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
```

and replace `keysOf`:

```ts
  const keysOf = (b: PluginBundle): string[] => {
    const name = fold(b.name);
    const keys = [`vn:${fold(b.vendor)}|${name}`];
    if (b.bundleId) keys.push(`id:${b.bundleId.toLowerCase()}|${name}`);
    return keys;
  };
```

Also update the comment above `mergePlugins` (lines 28-31): the vendor-spelling
example now merges via the fold, e.g. append one line: `// Vendor spellings that
differ only in case/punctuation ("Madrona Labs" vs "madronalabs") fold together.`

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test`
Expected: ALL tests PASS (new and pre-existing).

- [ ] **Step 5: Commit**

```bash
git add src/util.ts src/util.test.ts
git commit -m "Merge plugins across vendor/name spelling variants"
```

---

### Task 2: Grouping Rule B — family absorption pass

**Files:**
- Modify: `src/util.ts` (the whole `mergePlugins` body; helpers above it)
- Test: `src/util.test.ts`

**Interfaces:**
- Consumes: Task 1's `fold`.
- Produces: `mergePlugins` unchanged signature. New module-private helpers `tokensOf(name: string): string[]` and `digitsOf(tokens: string[]): string`.

- [ ] **Step 1: Write the failing tests**

Append inside `describe("mergePlugins", ...)` in `src/util.test.ts`:

```ts
  it("absorbs family variants into the base product (VCV Rack 2)", () => {
    const plugins = mergePlugins([
      mk({ id: "au", format: "AU", name: "VCV Rack 2", vendor: "VCV", bundleId: "com.vcvrack.rack" }),
      mk({ id: "fx", format: "AU", name: "VCV Rack 2 FX", vendor: "VCV", bundleId: "com.vcvrack.rack" }),
      mk({ id: "v3", format: "VST3", name: "VCV Rack 2", vendor: "vcvrack", bundleId: "com.vcvrack.rack" }),
      mk({ id: "cl", format: "CLAP", name: "VCV Rack 2", vendor: "vcvrack", bundleId: "com.vcvrack.rack" }),
      mk({ id: "app", format: "APP", name: "VCV Rack 2 Pro", vendor: "vcvrack", bundleId: "com.vcvrack.rackpro" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].name).toBe("VCV Rack 2");
    expect(plugins[0].installs).toHaveLength(5);
  });

  it("absorbs suffix-word companions (Serum FX) but not digit siblings (Serum 2)", () => {
    const plugins = mergePlugins([
      mk({ id: "s", name: "Serum", vendor: "Xfer Records", bundleId: "com.xferrecords.serum" }),
      mk({ id: "sfx", name: "Serum FX", vendor: "Xfer Records", bundleId: "com.xferrecords.serumfx" }),
      mk({ id: "s2", name: "Serum 2", vendor: "Xfer Records", bundleId: "com.xferrecords.serum2" }),
      mk({ id: "s2fx", name: "Serum 2 FX", vendor: "Xfer Records", bundleId: "com.xferrecords.serum2fx" }),
    ]);
    expect(plugins).toHaveLength(2);
    const names = plugins.map((p) => p.name).sort();
    expect(names).toEqual(["Serum", "Serum 2"]);
  });

  it("absorbs vendor-prefixed names (FabFilter Pro-Q 3) but not digit siblings", () => {
    const plugins = mergePlugins([
      mk({ id: "q3", name: "Pro-Q 3", vendor: "FabFilter", bundleId: "com.fabfilter.proq3" }),
      mk({ id: "q3f", name: "FabFilter Pro-Q 3", vendor: "FabFilter", bundleId: "com.fabfilter.proq3.full" }),
      mk({ id: "q4", name: "Pro-Q 4", vendor: "FabFilter", bundleId: "com.fabfilter.proq4" }),
    ]);
    expect(plugins).toHaveLength(2);
  });

  it("does not merge sibling products whose names only overlap (Ozone, Kontakt)", () => {
    const plugins = mergePlugins([
      mk({ id: "oe", name: "Ozone 11 Equalizer", vendor: "iZotope", bundleId: "com.izotope.ozone11eq" }),
      mk({ id: "oi", name: "Ozone 11 Imager", vendor: "iZotope", bundleId: "com.izotope.ozone11img" }),
      mk({ id: "k7", name: "Kontakt 7", vendor: "Native Instruments", bundleId: "com.ni.kontakt7" }),
      mk({ id: "k8", name: "Kontakt 8", vendor: "Native Instruments", bundleId: "com.ni.kontakt8" }),
    ]);
    expect(plugins).toHaveLength(4);
  });

  it("ignores format markers when comparing family names", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Kontakt 7", vendor: "Native Instruments", bundleId: "com.ni.kontakt7" }),
      mk({ id: "b", name: "Kontakt 7 (VST3)", vendor: "Native Instruments", bundleId: "com.ni.kontakt7.vst3" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].name).toBe("Kontakt 7");
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- util`
Expected: the five new tests FAIL (except possibly the Ozone/Kontakt negative test, which passes trivially — that's fine, it's a regression guard); pre-existing tests PASS.

- [ ] **Step 3: Implement the absorption pass**

In `src/util.ts`, add these module-level helpers directly above `mergePlugins`:

```ts
// Format markers carry no product identity ("Kontakt 7 (VST3)" is Kontakt 7)
// and must not contribute digits to the version-sibling check.
const FORMAT_TOKENS = new Set(["vst", "vst2", "vst3", "au", "audiounit", "aax", "clap", "app"]);

// Alphanumeric runs minus format markers, split into letter/digit subtokens:
// "Kontakt 7 (VST3)" → ["kontakt", "7"]; "Serum2" → ["serum", "2"].
const tokensOf = (name: string): string[] =>
  (name.toLowerCase().match(/[a-z0-9]+/g) ?? [])
    .filter((run) => !FORMAT_TOKENS.has(run))
    .flatMap((run) => run.match(/[a-z]+|\d+/g) ?? []);

// Digit changes mean a different product (Serum vs Serum 2, Pro-Q 3 vs 4).
const digitsOf = (tokens: string[]): string =>
  tokens.filter((t) => /^\d+$/.test(t)).join(".");

// The reverse-DNS org ("com.vcvrack") bridges a family whose members carry
// different bundle ids and vendor spellings across formats.
const orgOf = (bundleId: string): string | null => {
  const seg = bundleId.toLowerCase().split(".");
  return seg.length >= 2 ? `${seg[0]}.${seg[1]}` : null;
};

// Shortest name (fewest tokens, then fewest characters) is the base product
// name of a family — "VCV Rack 2", not "VCV Rack 2 MIDI FX".
const canonicalName = (list: PluginBundle[]): string => {
  let best = list[0].name;
  for (const b of list) {
    const [n, bn] = [tokensOf(b.name).length, tokensOf(best).length];
    if (n < bn || (n === bn && b.name.length < best.length)) best = b.name;
  }
  return best;
};
```

Then replace the body of `mergePlugins` between the first union-find loop and
the `plugins` mapping with a grouped + absorbed version. The complete new
function:

```ts
export function mergePlugins(bundles: PluginBundle[]): Plugin[] {
  const fold = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
  const parent = new Map<string, string>();
  const find = (k: string): string => {
    const p = parent.get(k) ?? k;
    if (p === k) return k;
    const root = find(p);
    parent.set(k, root);
    return root;
  };
  const union = (a: string, b: string) => {
    const ra = find(a);
    const rb = find(b);
    if (ra !== rb) parent.set(rb, ra);
  };
  const keysOf = (b: PluginBundle): string[] => {
    const name = fold(b.name);
    const keys = [`vn:${fold(b.vendor)}|${name}`];
    if (b.bundleId) keys.push(`id:${b.bundleId.toLowerCase()}|${name}`);
    return keys;
  };
  for (const b of bundles) {
    const [first, ...rest] = keysOf(b);
    for (const k of rest) union(first, k);
  }
  const map = new Map<string, PluginBundle[]>();
  for (const b of bundles) {
    const key = find(keysOf(b)[0]);
    const list = map.get(key);
    if (list) list.push(b);
    else map.set(key, [b]);
  }

  // Family absorption: "Serum FX", "VCV Rack 2 Pro" belong to the product
  // whose name theirs extends. Merge group EXT into group BASE when they
  // share an identity (vendor, bundle id, or reverse-DNS org), their digit
  // sequences match (digit changes are different products), and BASE's name
  // tokens are a subset of EXT's. Equal sets are allowed — "Kontakt 7" and
  // "Kontakt 7 (VST3)" tokenize identically once format markers drop.
  interface Fam { key: string; tokens: Set<string>; digits: string; identity: Set<string> }
  const fams: Fam[] = [...map.entries()].map(([key, list]) => {
    const tokens = new Set(tokensOf(canonicalName(list)));
    const identity = new Set<string>();
    for (const b of list) {
      identity.add(`v:${fold(b.vendor)}`);
      if (b.bundleId) {
        identity.add(`b:${b.bundleId.toLowerCase()}`);
        const org = orgOf(b.bundleId);
        if (org) identity.add(`o:${org}`);
      }
    }
    return { key, tokens: tokens, digits: digitsOf([...tokens]), identity };
  });
  for (const base of fams) {
    for (const ext of fams) {
      if (base === ext || base.digits !== ext.digits) continue;
      if (base.tokens.size > ext.tokens.size) continue;
      if (![...base.tokens].every((t) => ext.tokens.has(t))) continue;
      if (![...base.identity].some((i) => ext.identity.has(i))) continue;
      union(base.key, ext.key);
    }
  }
  const groups = new Map<string, PluginBundle[]>();
  for (const [key, list] of map) {
    const root = find(key);
    const existing = groups.get(root);
    if (existing) existing.push(...list);
    else groups.set(root, [...list]);
  }

  const plugins: Plugin[] = [...groups.entries()].map(([key, list]) => {
    const installs = [...list].sort(
      (a, b) => FORMATS.indexOf(a.format) - FORMATS.indexOf(b.format),
    );
    const scopes = new Set(installs.map((b) => b.scope));
    return {
      key,
      name: canonicalName(installs),
      vendor: installs[0].vendor,
      version: displayVersion(installs),
      installs,
      sizeBytes: installs.reduce((n, b) => n + b.sizeBytes, 0),
      scopes: SCOPE_ORDER.filter((s) => scopes.has(s)),
      // AU carries category; any install may carry copyright — take the first present.
      category: installs.find((b) => b.category)?.category ?? null,
      copyright: installs.find((b) => b.copyright)?.copyright ?? null,
    };
  });
  return plugins.sort(byName);
}
```

Note `digitsOf` in the Fam construction receives the token SET — digit
*sequence* degrades to digit *set* per group. That is intentional and safe:
tokens come from one canonical name, and duplicate digit tokens in one name
("Rack 2 2") don't occur in practice.

BEWARE the digit check uses the canonical name's tokens: for the VCV test,
group "VCV Rack 2 FX" has digits "2" and base "VCV Rack 2" has digits "2" —
equal, absorbed. "Serum 2 FX" has digits "2", base "Serum" has "" — not
absorbed into Serum; it IS absorbed into "Serum 2" (subset, digits equal).

`name` selection changed from `installs[0].name` to `canonicalName(installs)`.
One pre-existing test asserts `plugins[0].name` for same-named installs — the
canonical name equals that name, so it still passes.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test`
Expected: ALL tests PASS. If the pre-existing "prefers AU name" style test fails (it asserts on `installs[0]` ordering, not name selection), STOP and re-read — do not weaken existing assertions.

- [ ] **Step 5: Commit**

```bash
git add src/util.ts src/util.test.ts
git commit -m "Absorb family variants (FX/Pro/vendor-prefixed) into one plugin"
```

---

### Task 3: Vendor tern-engine with build-time model fetch

**Files:**
- Create: `src-tauri/vendor/tern-engine/` (copied from upstream, then modified)
- Create: `src-tauri/vendor/tern-engine/build.rs`
- Modify: `src-tauri/vendor/tern-engine/Cargo.toml` (de-workspace + build deps)
- Modify: `src-tauri/vendor/tern-engine/src/model.rs:37` (include path)
- Modify: `src-tauri/Cargo.toml` (add path dependency)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: crate `tern-engine` usable as `tern_engine::embed(text: &str) -> Vec<f32>` (384-dim, L2-normalized). Task 4 calls exactly this.

- [ ] **Step 1: Copy the engine crate at the pinned commit**

```bash
cd "$(mktemp -d)"
git clone https://github.com/soycaporal/ternlight
cd ternlight && git checkout f67b04f99e7ee83782e3ad3c81bf4f3f0e0d0bd8
cd <repo-root>   # /Users/nikitagriaznov/Documents/Work/POC/plugout
mkdir -p src-tauri/vendor
cp -R "$OLDPWD/engine" src-tauri/vendor/tern-engine
cp "$OLDPWD/LICENSE" src-tauri/vendor/tern-engine/LICENSE
rm -rf src-tauri/vendor/tern-engine/.cargo src-tauri/vendor/tern-engine/tests src-tauri/vendor/tern-engine/examples
rm -f src-tauri/vendor/tern-engine/assets/model.bin src-tauri/vendor/tern-engine/assets/.gitkeep
```

(`.cargo/` forces wasm SIMD flags; `tests/` are wasm-parity tests — neither applies to the native vendored build. `assets/tokenizer.json` MUST remain — it is `include_bytes!`-ed by the tokenizer.)

- [ ] **Step 2: De-workspace the crate manifest and add build deps**

Replace `src-tauri/vendor/tern-engine/Cargo.toml` `[package]` section (it uses `*.workspace = true` inheritance that breaks outside the upstream workspace) and append `[build-dependencies]`. Full new file — keep the existing `[lib]`, `[features]`, `[dependencies]` sections exactly as they are, only `[package]` changes and `[build-dependencies]` is added:

```toml
[package]
name        = "tern-engine"
version     = "0.1.0"
edition     = "2021"
license     = "MIT"
description = "Vendored ternlight inference engine (native build). Upstream: github.com/soycaporal/ternlight @ f67b04f."

[build-dependencies]
ureq = "2"
sha2 = { version = "0.10", default-features = false }
```

- [ ] **Step 3: Add build.rs that fetches the pinned model**

Create `src-tauri/vendor/tern-engine/build.rs`:

```rust
//! Fetches the packed int8 model into OUT_DIR at build time. The model is a
//! release artifact (not in the upstream git repo), so the vendored crate
//! downloads it once and verifies a pinned hash; subsequent builds are offline.
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{env, fs, path::PathBuf};

const MODEL_URL: &str =
    "https://huggingface.co/wenshutang/ternlight/resolve/main/model-embedding-int8.bin";
const MODEL_SHA256: &str = "5b693903bfc57b1699ca2c3f1d87332801f53a89885867eb63f3f8fc6ccce399";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("model.bin");
    if let Ok(existing) = fs::read(&out) {
        if hex(&Sha256::digest(&existing)) == MODEL_SHA256 {
            return;
        }
    }
    let resp = ureq::get(MODEL_URL).call().expect("download tern model.bin");
    let mut body = Vec::new();
    resp.into_reader()
        .read_to_end(&mut body)
        .expect("read tern model.bin body");
    assert_eq!(
        hex(&Sha256::digest(&body)),
        MODEL_SHA256,
        "model.bin hash mismatch — refusing to embed an unverified model"
    );
    fs::write(&out, body).expect("write model.bin to OUT_DIR");
}
```

- [ ] **Step 4: Point model.rs at OUT_DIR**

In `src-tauri/vendor/tern-engine/src/model.rs` line 37, replace:

```rust
static MODEL_BYTES: &[u8] = include_bytes!("../assets/model.bin");
```

with:

```rust
static MODEL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.bin"));
```

- [ ] **Step 5: Add the dependency to the app crate**

In `src-tauri/Cargo.toml` `[dependencies]`, append:

```toml
tern-engine = { path = "vendor/tern-engine", features = ["emb_int8"] }
```

- [ ] **Step 6: Verify it builds and embeds**

```bash
cd src-tauri && cargo build --release
```

Expected: compiles cleanly (first run downloads ~8.7 MB). If the build fails inside tern-engine with a missing-feature `compile_error!`, the `features = ["emb_int8"]` line is missing or misspelled.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/vendor/tern-engine src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "Vendor tern-engine with build-time model fetch"
```

---

### Task 4: Search index module and Tauri commands

**Files:**
- Create: `src-tauri/src/search.rs`
- Modify: `src-tauri/src/commands.rs` (two new commands at the end)
- Modify: `src-tauri/src/lib.rs` (module, state, handler registration)
- Test: inline `#[cfg(test)]` in `src-tauri/src/search.rs`

**Interfaces:**
- Consumes: `tern_engine::embed(&str) -> Vec<f32>` (Task 3).
- Produces: Tauri commands `index_search(docs: Vec<SearchDoc>)` and `semantic_search(query: String) -> Vec<SearchHit>` where `SearchDoc { id: String, text: String }`, `SearchHit { id: String, score: f32 }` (serde field names `id`, `text`, `score`). Task 5's frontend invokes `"index_search"` with `{ docs }` and `"semantic_search"` with `{ query }`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/search.rs` with the types and a stub `top_hits`, plus tests:

```rust
//! Semantic search over scanned plugins. The scan indexes one document per
//! bundle ("name vendor category"); queries embed once and rank by dot
//! product (vectors are L2-normalized, so dot == cosine).
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub struct SearchDoc {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
}

/// Hits below this don't read as related — calibrated by measurement; short
/// query → name scores run low ("reverb" vs a reverb plugin ≈ 0.23–0.39).
pub const MIN_SCORE: f32 = 0.30;
pub const TOP_K: usize = 8;

/// Replaced wholesale by each `index_search`; shared with `semantic_search`.
#[derive(Default, Clone)]
pub struct SearchIndex(pub Arc<Mutex<Vec<(String, Vec<f32>)>>>);

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn top_hits(index: &[(String, Vec<f32>)], query: &[f32]) -> Vec<SearchHit> {
    Vec::new() // stub — filled in by the implementation step
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(entries: &[(&str, [f32; 2])]) -> Vec<(String, Vec<f32>)> {
        entries.iter().map(|(id, v)| (id.to_string(), v.to_vec())).collect()
    }

    #[test]
    fn ranks_by_score_and_applies_threshold() {
        let index = idx(&[("low", [0.1, 0.0]), ("mid", [0.5, 0.0]), ("high", [0.9, 0.0])]);
        let hits = top_hits(&index, &[1.0, 0.0]);
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["high", "mid"]); // "low" (0.1) is under MIN_SCORE
    }

    #[test]
    fn caps_results_at_top_k() {
        let index: Vec<(String, Vec<f32>)> =
            (0..20).map(|i| (format!("p{i}"), vec![0.5, 0.0])).collect();
        assert_eq!(top_hits(&index, &[1.0, 0.0]).len(), TOP_K);
    }

    #[test]
    fn embeddings_rank_topically_related_names_higher() {
        let q = tern_engine::embed("reverb");
        let reverb = tern_engine::embed("TAL-Reverb-4 TAL Software effect");
        let synth = tern_engine::embed("Serum Xfer Records instrument");
        assert!(dot(&q, &reverb) > dot(&q, &synth));
    }
}
```

Register the module in `src-tauri/src/lib.rs` (add `mod search;` after `mod scanner;` — needed for the tests to compile).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test search`
Expected: `ranks_by_score_and_applies_threshold` and `caps_results_at_top_k` FAIL (stub returns empty); `embeddings_rank_topically_related_names_higher` PASSES (it only needs Task 3).

- [ ] **Step 3: Implement top_hits**

Replace the stub in `src-tauri/src/search.rs`:

```rust
pub fn top_hits(index: &[(String, Vec<f32>)], query: &[f32]) -> Vec<SearchHit> {
    let mut hits: Vec<SearchHit> = index
        .iter()
        .map(|(id, v)| SearchHit { id: id.clone(), score: dot(v, query) })
        .filter(|h| h.score >= MIN_SCORE)
        .collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(TOP_K);
    hits
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test search`
Expected: all three PASS.

- [ ] **Step 5: Wire the Tauri commands**

Append to `src-tauri/src/commands.rs`:

```rust
/// Replace the semantic-search index with embeddings of `docs`. Embedding a
/// few hundred docs takes ~1s of pure CPU, so it runs on the blocking pool.
#[tauri::command]
pub async fn index_search(
    state: tauri::State<'_, crate::search::SearchIndex>,
    docs: Vec<crate::search::SearchDoc>,
) -> Result<(), String> {
    let index = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let vectors: Vec<(String, Vec<f32>)> = docs
            .into_iter()
            .map(|d| (d.id, tern_engine::embed(&d.text)))
            .collect();
        *index.lock().unwrap() = vectors;
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn semantic_search(
    state: tauri::State<'_, crate::search::SearchIndex>,
    query: String,
) -> Vec<crate::search::SearchHit> {
    let q = tern_engine::embed(&query);
    crate::search::top_hits(&state.0.lock().unwrap(), &q)
}
```

In `src-tauri/src/lib.rs`, chain `.manage(search::SearchIndex::default())` after `.plugin(tauri_plugin_process::init())`, and add the two commands to `tauri::generate_handler![...]`:

```rust
        .plugin(tauri_plugin_process::init())
        .manage(search::SearchIndex::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::plugin_details,
            commands::remove_items,
            commands::removal_preview,
            commands::reveal_in_finder,
            commands::index_search,
            commands::semantic_search,
        ])
```

- [ ] **Step 6: Verify the whole crate builds and tests pass**

Run: `cd src-tauri && cargo test && cargo build --release`
Expected: all tests PASS, build clean.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/search.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "Add semantic search index and Tauri commands"
```

---

### Task 5: Frontend API wrappers and post-scan indexing

**Files:**
- Modify: `src/api.ts`
- Modify: `src/api.mock.ts`
- Modify: `src/App.tsx` (one new effect)

**Interfaces:**
- Consumes: Task 4's commands (`"index_search"` / `"semantic_search"`).
- Produces: `indexSearch(docs: SearchDoc[]): Promise<void>`, `semanticSearch(query: string): Promise<SearchHit[]>`, types `SearchDoc { id: string; text: string }`, `SearchHit { id: string; score: number }` exported from `src/api.ts`. Task 6 calls `semanticSearch`.

- [ ] **Step 1: Add mocks**

Append to `src/api.mock.ts`:

```ts
export const mockIndexSearch = async (): Promise<void> => {};
export const mockSemanticSearch = async (): Promise<{ id: string; score: number }[]> => [];
```

- [ ] **Step 2: Add API wrappers**

In `src/api.ts`, extend the mock import list with `mockIndexSearch, mockSemanticSearch`, and append after `removalPreview`:

```ts
// Semantic search (vendored ternlight model in the Rust backend)
export interface SearchDoc {
  id: string;
  text: string;
}
export interface SearchHit {
  id: string;
  score: number;
}
export const indexSearch = (docs: SearchDoc[]) =>
  isTauri ? invoke<void>("index_search", { docs }) : mockIndexSearch();
export const semanticSearch = (query: string) =>
  isTauri ? invoke<SearchHit[]>("semantic_search", { query }) : mockSemanticSearch();
```

- [ ] **Step 3: Index bundles when a scan settles**

In `src/App.tsx`: add `indexSearch` to the `./api` import list and `CATEGORY_LABELS` to the type import line (it's a value import: `import { CATEGORY_LABELS } from "./types";` — keep the existing `import type` line separate). Then add this effect after the Escape-key effect (line ~121):

```tsx
  // Feed the semantic-search index once a scan settles. Failures are logged
  // and swallowed — search degrades to substring-only.
  useEffect(() => {
    if (loading || bundles.length === 0) return;
    const docs = bundles.map((b) => ({
      id: b.id,
      text: `${b.name} ${b.vendor}${b.category ? ` ${CATEGORY_LABELS[b.category]}` : ""}`,
    }));
    indexSearch(docs).catch((e) => console.warn("semantic index failed", e));
    // Re-index only when a scan completes, not on receipt enrichment churn.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading]);
```

- [ ] **Step 4: Verify typecheck and tests**

Run: `npm run build && npm test`
Expected: tsc clean, all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/api.ts src/api.mock.ts src/App.tsx
git commit -m "Index scanned plugins for semantic search"
```

---

### Task 6: "Related" results in the plugin list

**Files:**
- Modify: `src/App.tsx` (semantic query state + related plugins)
- Modify: `src/components/PluginList.tsx` (optional `related` prop + divider)
- Modify: `src/styles.css` (divider style)
- Test: `src/components/PluginList.test.tsx`

**Interfaces:**
- Consumes: `semanticSearch` from Task 5; `mergePlugins` from Task 2.
- Produces: `PluginList` accepts optional prop `related?: Plugin[]`.

- [ ] **Step 1: Write the failing component tests**

Append to `src/components/PluginList.test.tsx` (uses the file's existing `mk`, `baseProps`, imports):

```tsx
describe("related results", () => {
  it("renders a divider and related rows when related plugins are present", () => {
    const related = mergePlugins([mk({ id: "r1", name: "ValhallaVintageVerb", vendor: "Valhalla DSP" })]);
    render(<PluginList {...baseProps()} plugins={mergePlugins([mk({ id: "a" })])} related={related} />);
    expect(screen.getByText("Related matches")).toBeInTheDocument();
    expect(screen.getByText("ValhallaVintageVerb")).toBeInTheDocument();
  });

  it("renders no divider when there are no related plugins", () => {
    render(<PluginList {...baseProps()} plugins={mergePlugins([mk({ id: "a" })])} />);
    expect(screen.queryByText("Related matches")).not.toBeInTheDocument();
  });

  it("keeps the empty state only when both lists are empty", () => {
    const related = mergePlugins([mk({ id: "r1", name: "ValhallaVintageVerb", vendor: "Valhalla DSP" })]);
    render(<PluginList {...baseProps()} query="reverb" related={related} />);
    expect(screen.queryByText(/No plugins match/)).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- PluginList`
Expected: first and third tests FAIL (`related` prop doesn't exist yet, TS error is also acceptable as failure); second PASSES trivially.

- [ ] **Step 3: Implement PluginList changes**

In `src/components/PluginList.tsx`:

1. Add to `Props`: `related?: Plugin[];`
2. Extract the existing `p.plugins.map((pl) => {...})` row JSX into a local
   function inside `PluginList` so related rows render identically:

```tsx
  const renderRow = (pl: Plugin) => {
    const selCount = pl.installs.filter((b) => p.selected.has(b.id)).length;
    return (
      <tr
        key={pl.key}
        className={pl.key === p.inspectedKey ? "sel" : ""}
        onClick={() => p.onRowClick(pl)}
        onMouseEnter={() => prefetchDetails(pl)}
      >
        <td className="c-check" onClick={(e) => e.stopPropagation()}>
          <TriCheckbox
            checked={selCount === pl.installs.length}
            indeterminate={selCount > 0 && selCount < pl.installs.length}
            onChange={() => p.onTogglePlugin(pl)}
            label={`Select ${pl.name}`}
          />
        </td>
        <td className="c-name">
          <div className="name">{pl.name}</div>
          <div className="vendor">{pl.vendor}</div>
        </td>
        <td className="c-vendor">{pl.vendor}</td>
        <td className="c-chips">
          {pl.installs.map((b) => (
            <FormatChip
              key={b.id}
              format={b.format}
              selected={p.selected.has(b.id)}
              onToggle={() => p.onToggleInstall(b.id)}
            />
          ))}
        </td>
        <td className="c-version">{pl.version || "—"}</td>
        <td className="c-size">{formatBytes(pl.sizeBytes)}</td>
      </tr>
    );
  };
```

3. In `<tbody>`, replace the plugins map with `{p.plugins.map(renderRow)}`, and
   insert the related section between it and the empty-state row:

```tsx
        {p.related && p.related.length > 0 && (
          <>
            <tr className="related-divider">
              <td colSpan={6}>Related matches</td>
            </tr>
            {p.related.map(renderRow)}
          </>
        )}
```

4. Change the empty-state condition to
   `{!p.loading && p.plugins.length === 0 && !(p.related && p.related.length > 0) && (`

- [ ] **Step 4: Wire App.tsx**

In `src/App.tsx`: add `semanticSearch` to the `./api` import and `Plugin` is already imported. Add state + effect + memo after the `visible` memo (line ~144):

```tsx
  const [relatedHits, setRelatedHits] = useState<Map<string, number>>(new Map());

  // Semantic hits trail the substring filter: debounce the query, embed it in
  // the backend, keep scores per bundle id. Backend failure ⇒ empty ⇒ the UI
  // silently stays substring-only.
  useEffect(() => {
    const q = query.trim();
    if (q.length < 3) {
      setRelatedHits(new Map());
      return;
    }
    let cancelled = false;
    const t = window.setTimeout(() => {
      semanticSearch(q).then(
        (hits) => !cancelled && setRelatedHits(new Map(hits.map((h) => [h.id, h.score]))),
        () => !cancelled && setRelatedHits(new Map()),
      );
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [query]);

  const related = useMemo(() => {
    if (relatedHits.size === 0 || query.trim().length < 3) return [];
    const ql = query.toLowerCase();
    const candidates = bundles.filter(
      (b) =>
        relatedHits.has(b.id) &&
        !`${b.name} ${b.vendor}`.toLowerCase().includes(ql) &&
        (formatFilter === "ALL" || b.format === formatFilter) &&
        (scopeFilter === "ALL" || b.scope === scopeFilter),
    );
    const merged = mergePlugins(candidates);
    const score = (p: Plugin) => Math.max(...p.installs.map((b) => relatedHits.get(b.id) ?? 0));
    return merged.sort((a, b) => score(b) - score(a));
  }, [bundles, relatedHits, query, formatFilter, scopeFilter]);
```

Pass it to the list: `<PluginList ... related={related} ... />`.

- [ ] **Step 5: Style the divider**

Append to `src/styles.css` (match the file's existing custom-property names — inspect the top of the file; if it defines e.g. `--muted` or similar text-secondary token, use that variable instead of the fallback literal):

```css
.related-divider td {
  padding: 14px 12px 4px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--muted, #8a8a8e);
}
```

- [ ] **Step 6: Run tests + typecheck**

Run: `npm run build && npm test`
Expected: tsc clean, ALL tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/components/PluginList.tsx src/components/PluginList.test.tsx src/styles.css
git commit -m "Show semantically related plugins under search results"
```

---

### Task 7: Per-install checkboxes in the Inspector

**Files:**
- Modify: `src/components/Inspector.tsx`
- Modify: `src/App.tsx:239` (pass two props)
- Test: Create `src/components/Inspector.test.tsx`

**Interfaces:**
- Consumes: existing `toggleInstall(id)` and `selected` set in App.tsx.
- Produces: `Inspector` props gain `selected: Set<string>` and `onToggleInstall: (id: string) => void`.

- [ ] **Step 1: Write the failing test**

Create `src/components/Inspector.test.tsx`:

```tsx
// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Format, PluginBundle, Scope } from "../types";
import { mergePlugins } from "../util";
import { Inspector } from "./Inspector";

vi.mock("../detailsCache", () => ({
  getDetails: vi.fn(() => new Promise(() => {})), // never resolves — cards show skeletons
}));
vi.mock("../api", () => ({ revealInFinder: vi.fn() }));

afterEach(cleanup);

function mk(over: Partial<PluginBundle> & { id: string }): PluginBundle {
  return {
    name: "Acid V",
    vendor: "Arturia",
    version: "1.0.0",
    format: "VST3" as Format,
    bundleId: "com.arturia.acid",
    path: `/Library/${over.id}`,
    sizeBytes: 10,
    scope: "system" as Scope,
    packageId: null,
    category: null,
    copyright: null,
    ...over,
  };
}

describe("Inspector install selection", () => {
  it("shows a checkbox per install reflecting the selection set", () => {
    const plugin = mergePlugins([mk({ id: "au1", format: "AU" }), mk({ id: "v31", format: "VST3" })])[0];
    render(
      <Inspector
        plugin={plugin}
        selected={new Set(["au1"])}
        onToggleInstall={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const boxes = screen.getAllByRole("checkbox");
    expect(boxes).toHaveLength(2);
    expect(boxes[0]).toBeChecked(); // AU sorts first (FORMATS order)
    expect(boxes[1]).not.toBeChecked();
  });

  it("reports toggles with the install's bundle id", async () => {
    const onToggle = vi.fn();
    const plugin = mergePlugins([mk({ id: "au1", format: "AU" }), mk({ id: "v31", format: "VST3" })])[0];
    render(
      <Inspector plugin={plugin} selected={new Set()} onToggleInstall={onToggle} onClose={vi.fn()} />,
    );
    await userEvent.click(screen.getAllByRole("checkbox")[1]);
    expect(onToggle).toHaveBeenCalledWith("v31");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- Inspector`
Expected: FAIL — `Inspector` has no `selected`/`onToggleInstall` props and renders no checkboxes.

- [ ] **Step 3: Implement**

In `src/components/Inspector.tsx`:

1. Extend `InstallCard` to accept and render the checkbox in its header:

```tsx
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
          aria-label={`Select ${bundle.format} install`}
          checked={checked}
          onChange={onToggle}
        />
        <FormatBadge format={bundle.format} />
        <span className="install-version">v{bundle.version || "?"}</span>
        <span className="install-size">{formatBytes(bundle.sizeBytes)}</span>
      </header>
      ...rest unchanged...
```

2. Extend `Inspector`'s signature and pass through:

```tsx
export function Inspector({
  plugin,
  selected,
  onToggleInstall,
  onClose,
}: {
  plugin: Plugin;
  selected: Set<string>;
  onToggleInstall: (id: string) => void;
  onClose: () => void;
}) {
```

and in the render:

```tsx
      {plugin.installs.map((b) => (
        <InstallCard
          key={b.id}
          bundle={b}
          details={details[b.id]}
          checked={selected.has(b.id)}
          onToggle={() => onToggleInstall(b.id)}
        />
      ))}
```

3. In `src/App.tsx:239`, pass the props:

```tsx
          {inspected && (
            <Inspector
              plugin={inspected}
              selected={selected}
              onToggleInstall={toggleInstall}
              onClose={() => setInspectedKey(null)}
            />
          )}
```

- [ ] **Step 4: Run tests + typecheck**

Run: `npm run build && npm test`
Expected: tsc clean, ALL tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/Inspector.tsx src/components/Inspector.test.tsx src/App.tsx
git commit -m "Select individual installs from the inspector"
```

---

### Task 8: Docs sync and end-to-end verification

**Files:**
- Modify: `docs/superpowers/specs/2026-07-11-semantic-search-grouping-design.md` (drop the stale shared-path section)
- Modify: `CHANGELOG.md` (new entry at top, match the file's existing format)

**Interfaces:** none.

- [ ] **Step 1: Correct the spec**

The spec's "Shared-path bookkeeping" section is wrong: the scanner emits one bundle per file and a bundle's id IS its path, so no two bundles share a path. Delete that whole subsection and add one sentence where it was: `Bundle ids are filesystem paths and unique per bundle — family groups never contain duplicate paths, so no dedup is needed.`

- [ ] **Step 2: Add a CHANGELOG entry**

Open `CHANGELOG.md`, copy the structure of the topmost release entry, and add an Unreleased/next-version entry describing: semantic "Related matches" in search, spelling/family variants merging into one plugin row, per-install selection in the inspector.

- [ ] **Step 3: End-to-end verification in the running app**

Run: `npm run tauri dev` (from the repo root; requires the Rust build from Task 4).
Verify, then quit the app:

1. sumu appears as ONE row (AU + VST3), not two.
2. VCV Rack 2 appears as ONE row containing the AU/VST3/CLAP installs and the Pro app.
3. Searching `reverb` shows substring matches first, then a "Related matches" divider with semantically related plugins (with only ~2 candidate reverbs installed the section may be short — presence of the divider is enough; it must NOT appear for `query.length < 3`).
4. Clicking a row opens the Inspector; each install card has a checkbox; checking only the VST3 install shows the row checkbox indeterminate and the ActionBar counting one install.
5. Rescan still repopulates the list and search keeps working afterwards.

- [ ] **Step 4: Full test suites one last time**

Run: `npm run build && npm test && (cd src-tauri && cargo test)`
Expected: everything green.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-07-11-semantic-search-grouping-design.md CHANGELOG.md
git commit -m "Sync spec and changelog with shipped search and grouping"
```
