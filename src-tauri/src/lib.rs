//! plugout backend: find and trash audio plugin bundles on macOS.
//!
//! The pipeline, in the order a scan flows through it:
//!
//! - [`scanner`] walks the AU/VST3/VST2/CLAP/AAX folders under both `Library`
//!   roots, reading each bundle's `Info.plist`. Results stream to the UI one
//!   folder at a time, so the list fills while slower folders are still sizing.
//! - [`receipts`] resolves which installer package owns each bundle. Every
//!   lookup spawns `pkgutil` (~250ms), so this runs after the scan, on a small
//!   worker pool, and trails in as `receipt:update` events.
//! - [`remover`] moves bundles to the Trash, batched by scope: one Trash call
//!   for user files, one privileged call — one admin prompt — for everything
//!   in `/Library`, no matter how many plugins are selected.
//! - [`commands`] is the thin Tauri layer that wires these to the frontend.
//!
//! A bundle's `id` is its filesystem path — there is deliberately no other
//! identity. External effects (`pkgutil`, the Trash) sit behind the
//! [`receipts::PkgUtil`] and [`remover::Trasher`] traits, so every module's
//! logic is tested without touching the real system.

mod commands;
mod model;
mod receipts;
mod remover;
mod scanner;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::plugin_details,
            commands::remove_items,
            commands::reveal_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
