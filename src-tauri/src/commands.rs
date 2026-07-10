use crate::model::{Format, PluginBundle, PluginDetails, RemovalResult, Scope};
use crate::receipts::{self, PkgUtil, RealPkgUtil};
use crate::remover::{self, RealTrasher};
use crate::reversal::{self, RealFs, RemovalPreview};
use crate::scanner;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
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
        let mut plugins: Vec<(String, String, String)> = Vec::new(); // (id, name, vendor)
        for (dir, format, scope) in scanner::plugin_locations() {
            let batch = scanner::scan_dir(&dir, format, scope);
            plugins.extend(
                batch
                    .iter()
                    .map(|b| (b.id.clone(), b.name.clone(), b.vendor.clone())),
            );
            let _ = app.emit("scan:batch", &batch);
        }
        let _ = app.emit("scan:done", plugins.len());
        let owners = enrich_receipts(&app, plugins.iter().map(|p| p.0.clone()).collect());
        discover_apps(&app, &plugins, &owners);
    });
}

/// Resolve each plugin's installer package in parallel, emitting `receipt:update`
/// events carrying batches of results (not one per plugin) so the UI re-renders a
/// couple dozen times rather than hundreds. Bounded to `RECEIPT_WORKERS` concurrent
/// `pkgutil` spawns. Returns every id → package pair for the app-discovery pass.
fn enrich_receipts(app: &AppHandle, ids: Vec<String>) -> Vec<(String, String)> {
    let queue = Arc::new(Mutex::new(ids));
    let owners = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for _ in 0..RECEIPT_WORKERS {
        let app = app.clone();
        let queue = Arc::clone(&queue);
        let owners = Arc::clone(&owners);
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
                        if let Some(pkg) = &package_id {
                            owners.lock().unwrap().push((id.clone(), pkg.clone()));
                        }
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
    Arc::try_unwrap(owners)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default()
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

/// Find companion applications and stream them as APP-format bundles: apps
/// written by any receipt in the plugins' package *families* (installers split
/// one product into `.vst3`/`.standalone`/`.resources`/… receipts), plus
/// same-name apps in the Applications folders — top-level or one vendor folder
/// deep. Apps inherit the linked plugin's vendor so they merge onto its row.
fn discover_apps(
    app: &AppHandle,
    plugins: &[(String, String, String)],
    owners: &[(String, String)],
) {
    let home = std::env::var("HOME").unwrap_or_default();
    let all_pkgs = RealPkgUtil.all_packages();
    let vendor_of: BTreeMap<&str, &str> = plugins
        .iter()
        .map(|(id, _, vendor)| (id.as_str(), vendor.as_str()))
        .collect();

    // Vendor hint per family package, from the plugin that led us to the family.
    let mut vendor_of_pkg: BTreeMap<String, String> = BTreeMap::new();
    for (plugin_id, pkg) in owners {
        let vendor = vendor_of
            .get(plugin_id.as_str())
            .copied()
            .unwrap_or_default();
        for member in reversal::expand_family(pkg, &all_pkgs) {
            vendor_of_pkg
                .entry(member)
                .or_insert_with(|| vendor.to_string());
        }
    }

    let pre = prefetch(vendor_of_pkg.keys().cloned().collect(), all_pkgs);

    // (app path → vendor hint, owning package)
    let mut found: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for (pkg, vendor) in &vendor_of_pkg {
        for path in reversal::apps_in(&reversal::package_paths(pkg, &pre)) {
            found
                .entry(path)
                .or_insert((vendor.clone(), Some(pkg.clone())));
        }
    }
    for (_, name, vendor) in plugins {
        for root in [format!("{home}/Applications"), "/Applications".to_string()] {
            for candidate in [
                format!("{root}/{name}.app"),
                format!("{root}/{vendor}/{name}.app"),
            ] {
                if Path::new(&candidate).exists() {
                    found.entry(candidate).or_insert((vendor.clone(), None));
                }
            }
        }
    }

    let batch: Vec<PluginBundle> = found
        .into_iter()
        .filter(|(path, _)| Path::new(path).exists())
        .map(|(path, (vendor, package_id))| app_bundle(&path, vendor, package_id, &home))
        .collect();
    if !batch.is_empty() {
        let _ = app.emit("scan:batch", &batch);
    }
}

fn app_bundle(path: &str, vendor: String, package_id: Option<String>, home: &str) -> PluginBundle {
    let meta = scanner::parse_info_plist(&Path::new(path).join("Contents/Info.plist"));
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let package_id = package_id.or_else(|| receipts::owner_of(path, &RealPkgUtil));
    PluginBundle {
        id: path.to_string(),
        name: if meta.name.is_empty() {
            stem
        } else {
            meta.name
        },
        vendor: if vendor.is_empty() {
            meta.vendor
        } else {
            vendor
        },
        version: meta.version,
        format: Format::App,
        bundle_id: meta.bundle_id,
        path: path.to_string(),
        size_bytes: scanner::dir_size(Path::new(path)),
        scope: if path.starts_with(&format!("{home}/")) {
            Scope::User
        } else {
            Scope::System
        },
        package_id,
    }
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
