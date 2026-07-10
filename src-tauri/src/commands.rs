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

/// Kick off a scan. Returns immediately; the work runs off the main thread so the
/// window never freezes. Emits:
///   - `scan:batch`  — a `Vec<PluginBundle>` for each plugin folder, as it's scanned
///   - `scan:done`   — the total count, once all folders are scanned
///   - `receipt:update` — `{ id, packageId }` per plugin, as installers resolve in the
///     background (each needs a `pkgutil` spawn, so this trails the fast scan)
#[tauri::command]
pub fn start_scan(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let mut plugins: Vec<PluginBundle> = Vec::new();
        for (dir, format, scope) in scanner::plugin_locations() {
            let batch = scanner::scan_dir(&dir, format, scope);
            let _ = app.emit("scan:batch", &batch);
            plugins.extend(batch);
        }
        // Companion apps come from a filesystem walk, so they land right after
        // the plugin folders — receipt enrichment below fills in their
        // "Installed by" like any other bundle.
        let apps = scanner::scan_applications(&scanner::application_roots(), &plugins);
        let _ = app.emit("scan:batch", &apps);
        let _ = app.emit("scan:done", plugins.len() + apps.len());

        let ids = plugins.iter().chain(&apps).map(|b| b.id.clone()).collect();
        enrich_receipts(&app, ids);
        let _ = app.emit("enrich:done", ());
    });
}

/// Resolve each bundle's installer package in parallel (plugins and apps
/// alike), emitting `receipt:update` events carrying batches of results so the
/// UI re-renders a couple dozen times rather than hundreds. Bounded to
/// `RECEIPT_WORKERS` concurrent `pkgutil` spawns.
fn enrich_receipts(app: &AppHandle, ids: Vec<String>) {
    let queue = Arc::new(Mutex::new(ids));
    let mut handles = Vec::new();
    for _ in 0..RECEIPT_WORKERS {
        let app = app.clone();
        let queue = Arc::clone(&queue);
        handles.push(std::thread::spawn(move || {
            let mut buf: Vec<ReceiptUpdate> = Vec::new();
            loop {
                let id = {
                    let mut q = queue.lock().unwrap();
                    q.pop()
                };
                match id {
                    Some(id) => {
                        let package_id = receipts::owner_of(&id, &RealPkgUtil);
                        buf.push(ReceiptUpdate { id, package_id });
                        if buf.len() >= RECEIPT_BATCH {
                            let _ = app.emit("receipt:update", &buf);
                            buf.clear();
                        }
                    }
                    None => {
                        if !buf.is_empty() {
                            let _ = app.emit("receipt:update", &buf);
                        }
                        break;
                    }
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
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
        handles.push(std::thread::spawn(move || loop {
            let Some(pkg) = queue.lock().unwrap().pop() else {
                break;
            };
            let files = RealPkgUtil.files(&pkg);
            let root = RealPkgUtil.install_root(&pkg);
            let mut r = results.lock().unwrap();
            r.0.insert(pkg.clone(), files);
            r.1.insert(pkg, root);
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let (files, roots) = Arc::try_unwrap(results)
        .map(|m| m.into_inner().unwrap())
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
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Full real-system pipeline with timings (not run in CI):
    /// `cargo test full_pipeline -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn full_pipeline_probe() {
        let t0 = Instant::now();
        let mut plugins: Vec<PluginBundle> = Vec::new();
        for (dir, format, scope) in scanner::plugin_locations() {
            plugins.extend(scanner::scan_dir(&dir, format, scope));
        }
        println!("scan: {} plugins in {:?}", plugins.len(), t0.elapsed());

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
