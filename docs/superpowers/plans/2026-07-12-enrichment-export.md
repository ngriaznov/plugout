# Corpus Enrichment + Inventory Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make semantic search answer function-word queries ("reverb", "equalizer") via curated keyword enrichment, and add a one-click CSV+JSON inventory export to Downloads.

**Architecture:** Enrichment is a static table in a new `src-tauri/src/keywords.rs`, applied to doc text inside `index_search`. Export is a pure `src/export.ts` (testable builders) plus a small `save_export` Tauri command and a toolbar button.

**Tech Stack:** Rust (Tauri 2), React 18 + TypeScript, vitest.

Spec: `docs/superpowers/specs/2026-07-12-enrichment-export-design.md` (the keyword curation lists live there — use them verbatim as the starting table).

## Global Constraints

- Never add AI/Claude attribution to commits, code, or docs.
- Branch: `enrichment-export`. Do NOT touch main until the release task.
- Before EVERY Rust-touching commit run, from `src-tauri/`: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` — all clean. (CI only runs after merge to main; these are the same gates it applies.)
- Frontend gates: `npm run build && npm test` green at every commit.
- The vendored `src-tauri/vendor/tern-engine` crate must not be modified.
- Serde field names cross the IPC boundary verbatim — no rename attributes.

---

### Task 1: Keyword enrichment table and application

**Files:**
- Create: `src-tauri/src/keywords.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod keywords;` after `mod commands;`)
- Modify: `src-tauri/src/commands.rs` (apply in `index_search`)

**Interfaces:**
- Produces: `keywords::enrich(name: &str, vendor: &str) -> String` (space-joined deduped keywords, may be empty). Task 1 only; nothing else consumes it.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/keywords.rs` with a stub and tests (table content comes from the spec's "Initial curation" section — transcribe ALL vendor and name entries listed there):

```rust
//! Curated keywords appended to each search document before embedding, so
//! function-word queries ("reverb", "equalizer") can match plugins whose
//! names are brand words. Static data, additive matching, no IO.

/// Lowercase + strip non-alphanumerics (mirrors the frontend grouping fold).
fn fold(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Folded-vendor substring → keywords. Keys are distinctive substrings so
/// vendor spelling variants match ("d16group" hits "d16groupaudiosoftware").
const VENDOR_KEYWORDS: &[(&str, &str)] = &[
    ("arturia", "analog synthesizer vintage keys emulation"),
    // ... transcribe every vendor entry from the spec ...
];

/// Folded-name substring → keywords. Keys must be >= 4 chars.
const NAME_KEYWORDS: &[(&str, &str)] = &[
    ("proq", "equalizer"),
    // ... transcribe every name entry from the spec ...
];

pub fn enrich(name: &str, vendor: &str) -> String {
    String::new() // stub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_substring_matches_spelling_variants() {
        assert!(enrich("Drumazon 2", "D16 Group Audio Software").contains("drum machine"));
        assert!(enrich("Drumazon 2", "d16group").contains("drum machine"));
    }

    #[test]
    fn vendor_and_name_matches_are_additive_and_deduped() {
        let k = enrich("Pro-Q 3", "FabFilter");
        // vendor gives "equalizer compressor limiter filter mixing", name gives "equalizer"
        assert!(k.contains("equalizer") && k.contains("compressor"));
        assert_eq!(k.matches("equalizer").count(), 1, "keywords must be deduped");
    }

    #[test]
    fn unknown_vendor_and_name_yield_empty() {
        assert_eq!(enrich("Obscuritron", "Nobody Knows Ltd"), "");
    }

    #[test]
    fn short_name_keys_are_absent() {
        assert!(NAME_KEYWORDS.iter().all(|(k, _)| k.len() >= 4));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Register `mod keywords;` in `src-tauri/src/lib.rs` (after `mod commands;`). Run: `cd src-tauri && cargo test keywords`
Expected: first two tests FAIL (stub returns empty); the last two PASS.

- [ ] **Step 3: Implement enrich**

```rust
pub fn enrich(name: &str, vendor: &str) -> String {
    let (fname, fvendor) = (fold(name), fold(vendor));
    let mut out: Vec<&str> = Vec::new();
    let hits = VENDOR_KEYWORDS
        .iter()
        .filter(|(k, _)| fvendor.contains(k))
        .chain(NAME_KEYWORDS.iter().filter(|(k, _)| fname.contains(k)));
    for (_, words) in hits {
        for w in words.split_whitespace() {
            if !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out.join(" ")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test keywords` — all PASS.

- [ ] **Step 5: Apply in index_search**

In `src-tauri/src/commands.rs`, `index_search` currently maps
`(d.id, tern_engine::embed(&d.text))`. The doc text arrives as
`"{name} {vendor} {category}"` — the backend cannot split it reliably, so the
frontend must send fields. Change `SearchDoc` in `src-tauri/src/search.rs` to:

```rust
#[derive(Deserialize)]
pub struct SearchDoc {
    pub id: String,
    pub name: String,
    pub vendor: String,
    /// Human category label, empty when unknown.
    pub category: String,
}
```

and in `index_search` build the embed text:

```rust
            .map(|d| {
                let keywords = crate::keywords::enrich(&d.name, &d.vendor);
                let text = [d.name.as_str(), d.vendor.as_str(), d.category.as_str(), keywords.as_str()]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                (d.id, tern_engine::embed(&text))
            })
```

Frontend side, `src/api.ts`: change `SearchDoc` to `{ id: string; name: string; vendor: string; category: string }`, and in `src/App.tsx`'s indexing effect build docs as:

```tsx
    const docs = bundles.map((b) => ({
      id: b.id,
      name: b.name,
      vendor: b.vendor,
      category: b.category ? CATEGORY_LABELS[b.category] : "",
    }));
```

(The `text` field disappears — update `src/api.mock.ts` only if it references it; it doesn't.)

- [ ] **Step 6: Full gates**

Run: `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` then `npm run build && npm test` from the root. All clean/green.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/keywords.rs src-tauri/src/lib.rs src-tauri/src/commands.rs src-tauri/src/search.rs src/api.ts src/App.tsx
git commit -m "Enrich search documents with curated keywords"
```

---

### Task 2: Real-corpus verification of the enrichment (controller-assisted)

**Files:** none in-repo (scratchpad replica only); possibly Modify `src-tauri/src/keywords.rs` (table tuning).

**Interfaces:** consumes Task 1's `enrich`.

- [ ] **Step 1: Extend the replica**

The scratchpad clone at
`/private/tmp/claude-502/-Users-nikitagriaznov-Documents-Work-POC-plugout/9c6e451c-94ea-4d66-9bb4-9007b6f5d2a4/scratchpad/ternlight/engine`
has `examples/smoke.rs` replicating the adaptive pipeline against
`../../corpus.tsv` (real machine corpus: name TAB vendor TAB category). Add the
SAME enrichment to the replica: copy the final `fold`/tables/`enrich` from the
committed `keywords.rs` into the example (plain copy is fine — it's scratch),
and append `enrich(name, vendor)` output to each corpus text before embedding.

- [ ] **Step 2: Run and compare against the 2026-07-11 baseline**

Run: `cargo run --release --features emb_int8 --example smoke ../../corpus.tsv`
Required outcomes (from the spec):
- "equalizer" → SSL/Harrison channel-strip entries appear in the gated output.
- "reverb" → Eventide/TAL/Valhalla-class reverb-capable entries rank above Retromulator, or Retromulator drops out.
- "piano", "drum machine", "synth", "moog" → no regressions (same or more sensible rows).

- [ ] **Step 3: Tune if needed**

If an outcome fails, adjust table entries in `src-tauri/src/keywords.rs` (and re-copy into the replica), re-run cargo tests and the replica, repeat. Record the final before/after query table in the task report.

- [ ] **Step 4: Commit (only if the table changed during tuning)**

```bash
git add src-tauri/src/keywords.rs
git commit -m "Tune enrichment keywords against the real corpus"
```

---

### Task 3: Export builders (pure TS)

**Files:**
- Create: `src/export.ts`
- Test: `src/export.test.ts`

**Interfaces:**
- Consumes: `Plugin` from `src/types.ts`.
- Produces: `exportCsv(plugins: Plugin[]): string`, `exportJson(plugins: Plugin[]): string`. Task 4 consumes both.

- [ ] **Step 1: Write the failing tests**

Create `src/export.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { exportCsv, exportJson } from "./export";
import { mergePlugins } from "./util";
import type { Format, PluginBundle, Scope } from "./types";

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

describe("exportCsv", () => {
  it("emits a header and one row per install", () => {
    const plugins = mergePlugins([mk({ id: "a", format: "AU" }), mk({ id: "b" })]);
    const lines = exportCsv(plugins).trimEnd().split("\n");
    expect(lines[0]).toBe(
      "product,name,vendor,version,format,scope,category,sizeBytes,path,bundleId,packageId",
    );
    expect(lines).toHaveLength(3);
    expect(lines[1]).toContain("AU"); // installs in FORMATS order
  });

  it("quotes fields containing commas and doubles embedded quotes", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: 'Big, "Bad" Delay', vendor: "ACME, Inc" }),
    ]);
    const row = exportCsv(plugins).trimEnd().split("\n")[1];
    expect(row).toContain('"Big, ""Bad"" Delay"');
    expect(row).toContain('"ACME, Inc"');
  });

  it("handles an empty list (header only)", () => {
    expect(exportCsv([]).trimEnd()).toBe(
      "product,name,vendor,version,format,scope,category,sizeBytes,path,bundleId,packageId",
    );
  });
});

describe("exportJson", () => {
  it("nests installs under products", () => {
    const plugins = mergePlugins([mk({ id: "a", format: "AU" }), mk({ id: "b" })]);
    const parsed = JSON.parse(exportJson(plugins));
    expect(parsed).toHaveLength(1);
    expect(parsed[0].name).toBe("Acid V");
    expect(parsed[0].installs).toHaveLength(2);
    expect(parsed[0].installs[0].format).toBe("AU");
    expect(parsed[0].installs[0].path).toBe("/Library/a");
  });

  it("serializes an empty list as []", () => {
    expect(JSON.parse(exportJson([]))).toEqual([]);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- export` — FAIL (module doesn't exist).

- [ ] **Step 3: Implement**

Create `src/export.ts`:

```ts
import type { Plugin } from "./types";

const HEADER = [
  "product", "name", "vendor", "version", "format", "scope", "category",
  "sizeBytes", "path", "bundleId", "packageId",
] as const;

// RFC 4180: quote when the field contains a comma, quote, or newline.
const cell = (v: string | number | null): string => {
  const s = v === null ? "" : String(v);
  return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};

export function exportCsv(plugins: Plugin[]): string {
  const rows = plugins.flatMap((p) =>
    p.installs.map((b) =>
      [p.name, b.name, b.vendor, b.version, b.format, b.scope, b.category ?? "",
       b.sizeBytes, b.path, b.bundleId, b.packageId ?? ""].map(cell).join(","),
    ),
  );
  return [HEADER.join(","), ...rows].join("\n") + "\n";
}

export function exportJson(plugins: Plugin[]): string {
  const products = plugins.map((p) => ({
    name: p.name,
    vendor: p.vendor,
    version: p.version,
    sizeBytes: p.sizeBytes,
    category: p.category,
    installs: p.installs.map((b) => ({
      name: b.name,
      vendor: b.vendor,
      version: b.version,
      format: b.format,
      scope: b.scope,
      category: b.category,
      sizeBytes: b.sizeBytes,
      path: b.path,
      bundleId: b.bundleId,
      packageId: b.packageId,
    })),
  }));
  return JSON.stringify(products, null, 2) + "\n";
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test` — ALL green.

- [ ] **Step 5: Commit**

```bash
git add src/export.ts src/export.test.ts
git commit -m "Add CSV and JSON inventory builders"
```

---

### Task 4: save_export command and toolbar button

**Files:**
- Modify: `src-tauri/src/commands.rs` (+ `src-tauri/src/lib.rs` handler list)
- Modify: `src/api.ts`, `src/api.mock.ts`
- Modify: `src/App.tsx` (button + handler + toast)

**Interfaces:**
- Consumes: Task 3's `exportCsv`/`exportJson`; existing `revealInFinder`.
- Produces: command `save_export(files: Vec<ExportFile>) -> Result<String, String>`; TS wrapper `saveExport(files: { name: string; contents: string }[]): Promise<string>` returning the directory written to.

- [ ] **Step 1: Rust command**

Append to `src-tauri/src/commands.rs`:

```rust
#[derive(Deserialize)]
pub struct ExportFile {
    pub name: String,
    pub contents: String,
}

/// Write export files into the user's Downloads folder; returns that folder's
/// path so the frontend can reveal it. Rejects names with path separators.
#[tauri::command]
pub fn save_export(app: AppHandle, files: Vec<ExportFile>) -> Result<String, String> {
    use tauri::Manager;
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    for f in &files {
        if f.name.contains('/') || f.name.contains('\\') {
            return Err(format!("invalid export file name: {}", f.name));
        }
        std::fs::write(dir.join(&f.name), &f.contents).map_err(|e| e.to_string())?;
    }
    Ok(dir.to_string_lossy().into_owned())
}
```

Register `commands::save_export` in `src-tauri/src/lib.rs`'s `generate_handler![...]`.
Note `Deserialize` is already imported in commands.rs.

Rust test (same file's `mod tests`): none — the command is a thin fs wrapper
around the path resolver; name-validation logic is the only branch worth a
test and it needs no AppHandle:

Extract the guard into a testable helper and test it:

```rust
fn valid_export_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains('\\')
}
```

use it in the loop (`if !valid_export_name(&f.name) { return Err(...) }`), and add:

```rust
    #[test]
    fn export_names_reject_path_separators() {
        assert!(valid_export_name("plugout-inventory-2026-07-12.csv"));
        assert!(!valid_export_name("../evil.csv"));
        assert!(!valid_export_name("a\\b.csv"));
        assert!(!valid_export_name(""));
    }
```

- [ ] **Step 2: Rust gates**

Run: `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` — clean.

- [ ] **Step 3: TS wrapper + mock**

`src/api.mock.ts`: `export const mockSaveExport = async (): Promise<string> => "/tmp";`
`src/api.ts` (extend mock import list too):

```ts
export interface ExportFile {
  name: string;
  contents: string;
}
export const saveExport = (files: ExportFile[]) =>
  isTauri ? invoke<string>("save_export", { files }) : mockSaveExport();
```

- [ ] **Step 4: App.tsx button**

Imports: add `saveExport` to the `./api` list, `exportCsv, exportJson` from `./export`, and `revealInFinder` from `./api`. Add state + handler near the other handlers:

```tsx
  const [exportedDir, setExportedDir] = useState<string | null>(null);

  async function doExport() {
    const all = sortPlugins(mergePlugins(bundles), "name", 1);
    const stamp = new Date().toISOString().slice(0, 10);
    try {
      const dir = await saveExport([
        { name: `plugout-inventory-${stamp}.csv`, contents: exportCsv(all) },
        { name: `plugout-inventory-${stamp}.json`, contents: exportJson(all) },
      ]);
      setExportedDir(dir);
    } catch {
      setExportedDir("error");
    }
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setExportedDir(null), 6000);
  }
```

Toolbar (next to Rescan):

```tsx
          <button className="ghost small" onClick={doExport} disabled={loading || bundles.length === 0}>
            Export
          </button>
```

Toast (next to the existing removal-results toast; render when `exportedDir` is set and `results` is not):

```tsx
        {exportedDir && !results && (
          <div className={`toast${exportedDir === "error" ? " toast-error" : ""}`} role="status">
            {exportedDir === "error" ? (
              <strong>Export failed</strong>
            ) : (
              <>
                <strong>Inventory exported to Downloads</strong>{" "}
                <button className="ghost small" onClick={() => revealInFinder(exportedDir)}>
                  Reveal
                </button>
              </>
            )}
          </div>
        )}
```

Note: `revealInFinder` on a directory path opens that folder in Finder (the
backend uses the opener plugin's reveal; a directory is acceptable input — if
manual verification in Task 5 shows otherwise, pass the CSV file path instead:
`` `${exportedDir}/plugout-inventory-${stamp}.csv` `` kept in state).

- [ ] **Step 5: Gates + commit**

Run: `npm run build && npm test` — green.

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src/api.ts src/api.mock.ts src/App.tsx
git commit -m "Add inventory export to Downloads"
```

---

### Task 5: Release 0.2.5 (controller: oss-polishing sweep FIRST, then this task)

**Files:**
- Modify: `CHANGELOG.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- Possibly: files flagged by the oss-polishing sweep

**Interfaces:** none.

- [ ] **Step 0 (controller, before dispatching this task):** run the oss-polishing skill sweep over the branch; fold actionable findings into commits on the branch.

- [ ] **Step 1: CHANGELOG**

Add a `## 0.2.5` entry above `## 0.2.4`, matching the existing format: smarter related-search matching (curated keywords, so "reverb"/"equalizer" find relevant gear), inventory export (CSV + JSON to Downloads).

- [ ] **Step 2: Full verification**

`npm run build && npm test && (cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)` — all green. Launch check: `npm run tauri dev`, confirm app starts, Export button visible and enabled post-scan, then quit (GUI click-through is the user's).

- [ ] **Step 3: Version roll**

`src-tauri/Cargo.toml` version → `0.2.5`; `cd src-tauri && cargo update -p plugout --offline` to refresh the lock.

- [ ] **Step 4: Commits**

```bash
git add CHANGELOG.md && git commit -m "Changelog for 0.2.5"
git add src-tauri/Cargo.toml src-tauri/Cargo.lock && git commit -m "Release 0.2.5: keyword-enriched search and inventory export"
```

- [ ] **Step 5 (controller): merge + verify**

Controller merges `enrichment-export` into main (ff-only), pushes, then polls CI + Release workflow until green and tag `v0.2.5` exists.
