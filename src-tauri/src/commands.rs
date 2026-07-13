use crate::error::CmdError;
use crate::model::{PluginBundle, PluginDetails, RemovalResult};
use crate::receipts::{self, PkgUtil, RealPkgUtil};
use crate::remover::{self, RealTrasher};
use crate::reversal::{self, RealFs, RemovalPreview};
use crate::scanner;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptUpdate {
    id: String,
    package_id: Option<String>,
}

/// How many `pkgutil` processes to run at once during background receipt enrichment.
const RECEIPT_WORKERS: usize = 8;
/// How many resolved receipts to accumulate before emitting an update, so the UI
/// re-renders a couple dozen times during enrichment instead of once per plugin.
const RECEIPT_BATCH: usize = 20;

/// A pkgutil result collected off the worker pool for caching: bundle id, its
/// mtime at lookup time, and the resolved (or absent) owning package.
type ResolvedReceipt = (String, u64, Option<String>);

/// Kick off a scan of the standard plugin folders, any user-chosen
/// `extra_dirs`, and the Applications folders. Returns immediately; the work
/// runs off the main thread so the window never freezes. A bundle found under
/// more than one root (an extra dir overlapping a standard one, say) is kept
/// only once, by id. Emits:
///   - `scan:batch`  — a `Vec<PluginBundle>` per root as it's scanned, holding
///     only the ids not already seen in an earlier batch
///   - `scan:done`   — the deduped total, once every root is in
///   - `receipt:update` — `{ id, packageId }` per bundle, as installers resolve
///     in the background (each needs a `pkgutil` spawn, so this trails the scan)
///   - `enrich:done` — when the last receipt has resolved
#[tauri::command]
pub fn start_scan(app: AppHandle, extra_dirs: Vec<String>) {
    tauri::async_runtime::spawn_blocking(move || {
        let mut seen = std::collections::HashSet::new();
        let mut push_batch = |batch: Vec<PluginBundle>, plugins: &mut Vec<PluginBundle>| {
            let fresh: Vec<_> = batch
                .into_iter()
                .filter(|b| seen.insert(b.id.clone()))
                .collect();
            if !fresh.is_empty() {
                let _ = app.emit("scan:batch", &fresh);
                plugins.extend(fresh);
            }
        };

        let mut plugins: Vec<PluginBundle> = Vec::new();
        for (dir, format, scope) in scanner::plugin_locations() {
            push_batch(scanner::scan_dir(&dir, format, scope), &mut plugins);
        }
        for dir in &extra_dirs {
            push_batch(
                scanner::scan_extra_dir(std::path::Path::new(dir)),
                &mut plugins,
            );
        }
        // Companion apps come from a filesystem walk, so they land right after
        // the plugin folders — receipt enrichment below fills in their
        // "Installed by" like any other bundle.
        let apps = scanner::scan_applications(&scanner::application_roots(), &plugins);
        push_batch(apps, &mut plugins);
        let _ = app.emit("scan:done", plugins.len());

        let ids = plugins.iter().map(|b| b.id.clone()).collect();
        enrich_receipts(&app, ids);
        let _ = app.emit("enrich:done", ());
    });
}

fn cache_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("receipt-cache.json"))
}

/// Resolve each bundle's installer package, serving unchanged bundles from the
/// on-disk cache and running only misses through the pkgutil worker pool.
/// Emits `receipt:update` events carrying batches of results (cache hits
/// first, then pool results as they trail in) so the UI re-renders a couple
/// dozen times rather than hundreds. Bounded to `RECEIPT_WORKERS` concurrent
/// `pkgutil` spawns.
fn enrich_receipts(app: &AppHandle, ids: Vec<String>) {
    let path = cache_path(app);
    let mut cache = path
        .as_deref()
        .map(crate::receipt_cache::ReceiptCache::load)
        .unwrap_or_default();

    // Partition: cache hits emit immediately, misses go to the pool.
    let mut hits: Vec<ReceiptUpdate> = Vec::new();
    let mut misses: Vec<(String, u64)> = Vec::new();
    for id in ids {
        let Some(mtime) = crate::receipt_cache::bundle_mtime_ms(&id) else {
            continue; // vanished between scan and enrichment
        };
        match cache.lookup(&id, mtime) {
            Some(package_id) => hits.push(ReceiptUpdate { id, package_id }),
            None => misses.push((id, mtime)),
        }
    }
    if !hits.is_empty() {
        let _ = app.emit("receipt:update", &hits);
    }

    // Worker pool over misses; results collected so they can be cached.
    let queue = Arc::new(Mutex::new(misses));
    let resolved: Arc<Mutex<Vec<ResolvedReceipt>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for _ in 0..RECEIPT_WORKERS {
        let app = app.clone();
        let queue = Arc::clone(&queue);
        let resolved = Arc::clone(&resolved);
        handles.push(std::thread::spawn(move || {
            let mut buf: Vec<ReceiptUpdate> = Vec::new();
            while let Some((id, mtime)) = { queue.lock().unwrap_or_else(|e| e.into_inner()).pop() }
            {
                let package_id = receipts::owner_of(&id, &RealPkgUtil);
                resolved.lock().unwrap_or_else(|e| e.into_inner()).push((
                    id.clone(),
                    mtime,
                    package_id.clone(),
                ));
                buf.push(ReceiptUpdate { id, package_id });
                if buf.len() >= RECEIPT_BATCH {
                    let _ = app.emit("receipt:update", &buf);
                    buf.clear();
                }
            }
            if !buf.is_empty() {
                let _ = app.emit("receipt:update", &buf);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    let resolved = Arc::try_unwrap(resolved)
        .map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()))
        .unwrap_or_default();
    if let Some(path) = path
        && !resolved.is_empty()
    {
        for (id, mtime, package_id) in resolved {
            cache.insert(id, mtime, package_id);
        }
        let _ = cache.save(&path); // failure is non-fatal by design
    }
}

/// `pkgutil --files`/`--pkg-info` fetched up front on a worker pool for a set
/// of packages; implements [`PkgUtil`] so the pure reversal logic can consume
/// it without knowing about threads.
struct Prefetched {
    files: BTreeMap<String, Option<Vec<String>>>,
    roots: BTreeMap<String, Option<String>>,
    all: Vec<String>,
}

impl PkgUtil for Prefetched {
    fn file_info(&self, _path: &str) -> Option<Vec<String>> {
        None
    }
    fn files(&self, pkg_id: &str) -> Option<Vec<String>> {
        self.files.get(pkg_id).cloned().flatten()
    }
    fn install_root(&self, pkg_id: &str) -> Option<String> {
        self.roots.get(pkg_id).cloned().flatten()
    }
    fn all_packages(&self) -> Vec<String> {
        self.all.clone()
    }
}

fn prefetch(pkgs: BTreeSet<String>, all: Vec<String>) -> Prefetched {
    let queue = Arc::new(Mutex::new(pkgs.into_iter().collect::<Vec<_>>()));
    let results = Arc::new(Mutex::new((BTreeMap::new(), BTreeMap::new())));
    let mut handles = Vec::new();
    for _ in 0..RECEIPT_WORKERS {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        handles.push(std::thread::spawn(move || {
            loop {
                let Some(pkg) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop() else {
                    break;
                };
                let files = RealPkgUtil.files(&pkg);
                let root = RealPkgUtil.install_root(&pkg);
                let mut r = results.lock().unwrap_or_else(|e| e.into_inner());
                r.0.insert(pkg.clone(), files);
                r.1.insert(pkg, root);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let (files, roots) = Arc::try_unwrap(results)
        .map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()))
        .unwrap_or_default();
    Prefetched { files, roots, all }
}

/// Expand a set of owning packages to their full receipt families.
fn family_set(pkgs: impl IntoIterator<Item = String>, all: &[String]) -> BTreeSet<String> {
    pkgs.into_iter()
        .flat_map(|p| reversal::expand_family(&p, all))
        .collect()
}

/// Details for one plugin, resolved on demand when its drawer opens (one `pkgutil`
/// call). The frontend already holds the bundle, so this only returns what removal
/// would trash plus the installer package.
#[tauri::command]
pub async fn plugin_details(id: String) -> PluginDetails {
    // `pkgutil` takes hundreds of ms; run it off the IPC thread so the UI never stalls.
    tauri::async_runtime::spawn_blocking(move || {
        let package_id = receipts::owner_of(&id, &RealPkgUtil);
        PluginDetails {
            files_to_trash: vec![id],
            package_id,
        }
    })
    .await
    .unwrap_or(PluginDetails {
        files_to_trash: vec![],
        package_id: None,
    })
}

/// A bundle's id IS its path, so removal needs nothing but the ids.
#[tauri::command]
pub fn remove_items(ids: Vec<String>) -> Vec<RemovalResult> {
    let home = std::env::var("HOME").unwrap_or_default();
    remover::remove_paths(&ids, &home, &RealTrasher)
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), CmdError> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(CmdError::from)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedBundle {
    id: String,
    package_id: Option<String>,
}

/// What removing `removing` would additionally clean up. `bundles` is the
/// frontend's full scanned set (survivors included) — the backend is stateless,
/// and exclusivity can only be judged against everything installed.
#[tauri::command]
pub async fn removal_preview(removing: Vec<String>, bundles: Vec<OwnedBundle>) -> RemovalPreview {
    tauri::async_runtime::spawn_blocking(move || {
        let removing: BTreeSet<String> = removing.into_iter().collect();
        let all_owned: BTreeMap<String, String> = bundles
            .into_iter()
            .filter_map(|b| b.package_id.map(|p| (b.id, p)))
            .collect();
        // Prefetch the removal's package families in parallel; the pure logic
        // then runs entirely against the cache.
        let all_pkgs = RealPkgUtil.all_packages();
        let owner_pkgs = removing.iter().filter_map(|p| all_owned.get(p)).cloned();
        let pre = prefetch(family_set(owner_pkgs, &all_pkgs), all_pkgs);
        reversal::removal_preview(&removing, &all_owned, &pre, &RealFs)
    })
    .await
    .unwrap_or_default()
}

/// Replace the semantic-search index with embeddings of `docs`. Embedding a
/// few hundred docs takes ~1s of pure CPU, so it runs on the blocking pool.
#[tauri::command]
pub async fn index_search(
    state: tauri::State<'_, crate::search::SearchIndex>,
    docs: Vec<crate::search::SearchDoc>,
) -> Result<(), CmdError> {
    let index = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let vectors: Vec<(String, Vec<f32>)> = docs
            .into_iter()
            .map(|d| {
                let keywords = crate::keywords::enrich(&d.name, &d.vendor);
                let subcats = if d.id.to_ascii_lowercase().ends_with(".vst3") {
                    crate::vst3meta::subcategory_words(std::path::Path::new(&d.id)).join(" ")
                } else {
                    String::new()
                };
                let text = [
                    d.name.as_str(),
                    d.vendor.as_str(),
                    d.category.as_str(),
                    keywords.as_str(),
                    subcats.as_str(),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
                (d.id, tern_engine::embed(&text))
            })
            .collect();
        *index.lock().unwrap_or_else(|e| e.into_inner()) = vectors;
    })
    .await
    .map_err(|e| CmdError::Internal(e.to_string()))
}

#[tauri::command(async)]
pub fn semantic_search(
    state: tauri::State<'_, crate::search::SearchIndex>,
    query: String,
) -> Vec<crate::search::SearchHit> {
    let q = tern_engine::embed(&query);
    crate::search::top_hits(&state.0.lock().unwrap_or_else(|e| e.into_inner()), &q)
}

#[derive(Deserialize)]
pub struct ExportFile {
    pub name: String,
    pub contents: String,
}

/// Reject names with path separators or empty names, so exported files can
/// only ever land directly inside the target directory.
fn valid_export_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains('\\')
}

/// Write export files into the user's Downloads folder; returns that folder's
/// path so the frontend can reveal it. Rejects names with path separators.
#[tauri::command]
pub fn save_export(app: AppHandle, files: Vec<ExportFile>) -> Result<String, CmdError> {
    use tauri::Manager;
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| CmdError::Internal(e.to_string()))?;
    for f in &files {
        if !valid_export_name(&f.name) {
            return Err(CmdError::Internal(format!(
                "invalid export file name: {}",
                f.name
            )));
        }
        std::fs::write(dir.join(&f.name), &f.contents).map_err(CmdError::from)?;
    }
    Ok(dir.to_string_lossy().into_owned())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageHit {
    pub name: String,
    pub vendor: String,
    pub project: String,
    pub mtime_ms: f64,
}

/// Scan DAW project files for plugin references. IO-bound: runs on the
/// blocking pool. Unreadable files are skipped; all failure modes degrade to
/// an empty result. `known_names` is the set of installed plugin names, used
/// only for `.logicx` (its binary `ProjectData` carries no readable plugin
/// list, so we search for names we already know are installed).
#[tauri::command]
pub async fn scan_usage(known_names: Vec<String>) -> Result<Vec<UsageHit>, CmdError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut hits = Vec::new();
        for path in crate::usage::find_projects(&crate::usage::RealFinder) {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_dir() && meta.len() > 64 * 1024 * 1024 {
                continue;
            }
            let mtime_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0.0, |d| d.as_millis() as f64);
            let lower = path.to_string_lossy().to_lowercase();
            let refs = if lower.ends_with(".als") {
                std::fs::read(&path)
                    .map(|b| crate::usage::parse_als(&b))
                    .unwrap_or_default()
            } else if lower.ends_with(".song") {
                std::fs::read(&path)
                    .map(|b| crate::usage::parse_song(&b))
                    .unwrap_or_default()
            } else if lower.ends_with(".logicx") {
                crate::usage::parse_logic(&path, &known_names)
            } else {
                std::fs::read_to_string(&path)
                    .map(|t| crate::usage::parse_rpp(&t))
                    .unwrap_or_default()
            };
            let project = path.to_string_lossy().into_owned();
            hits.extend(refs.into_iter().map(|r| UsageHit {
                name: r.name,
                vendor: r.vendor,
                project: project.clone(),
                mtime_ms,
            }));
        }
        hits
    })
    .await
    .map_err(|e| CmdError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn export_names_reject_path_separators() {
        assert!(valid_export_name("plugout-inventory-2026-07-12.csv"));
        assert!(!valid_export_name("../evil.csv"));
        assert!(!valid_export_name("a\\b.csv"));
        assert!(!valid_export_name(""));
    }

    /// Full real-system pipeline with timings (not run in CI):
    /// `cargo test full_pipeline -- --ignored --nocapture`
    #[test]
    #[ignore = "real-system probe; run with `cargo test full_pipeline -- --ignored --nocapture`"]
    fn full_pipeline_probe() {
        let t0 = Instant::now();
        let mut plugins: Vec<PluginBundle> = Vec::new();
        for (dir, format, scope) in scanner::plugin_locations() {
            plugins.extend(scanner::scan_dir(&dir, format, scope));
        }
        let categorized = plugins.iter().filter(|b| b.category.is_some()).count();
        let with_copyright = plugins.iter().filter(|b| b.copyright.is_some()).count();
        println!(
            "scan: {} plugins in {:?} — {categorized} categorized, {with_copyright} with copyright",
            plugins.len(),
            t0.elapsed(),
        );

        let t1 = Instant::now();
        let apps = scanner::scan_applications(&scanner::application_roots(), &plugins);
        println!(
            "scan_applications: {} apps in {:?}",
            apps.len(),
            t1.elapsed()
        );
        for a in apps.iter().take(10) {
            println!("  APP {} ({})", a.path, a.vendor);
        }
        assert!(
            !apps.is_empty(),
            "expected at least one companion app on this machine"
        );
    }
}
