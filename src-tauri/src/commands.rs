use crate::model::{PluginDetails, RemovalResult};
use crate::receipts::{self, RealPkgUtil};
use crate::remover::{self, RealTrasher};
use crate::scanner;
use serde::Serialize;
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
        let mut ids: Vec<String> = Vec::new();
        for (dir, format, scope) in scanner::plugin_locations() {
            let batch = scanner::scan_dir(&dir, format, scope);
            ids.extend(batch.iter().map(|b| b.id.clone()));
            let _ = app.emit("scan:batch", &batch);
        }
        let _ = app.emit("scan:done", ids.len());
        enrich_receipts(&app, ids);
    });
}

/// Resolve each plugin's installer package in parallel, emitting `receipt:update`
/// events carrying batches of results (not one per plugin) so the UI re-renders a
/// couple dozen times rather than hundreds. Bounded to `RECEIPT_WORKERS` concurrent
/// `pkgutil` spawns.
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
