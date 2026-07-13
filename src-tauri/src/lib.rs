//! plugout backend: find and trash audio plugins and their leftovers on macOS.
//!
//! The pipeline, in the order a scan flows through it:
//!
//! - [`scanner`] walks the AU/VST3/VST2/CLAP/AAX folders under both `Library`
//!   roots, reading each bundle's `Info.plist`, then walks the Applications
//!   folders for companion apps linked to those plugins by name, bundle-id
//!   vendor, or vendor folder. Results stream to the UI as they're found.
//! - [`receipts`] resolves which installer package owns each bundle. Every
//!   lookup spawns `pkgutil` (~250ms), so this runs after the scan, on a small
//!   worker pool, and trails in as `receipt:update` events.
//! - [`reversal`] answers "what else did the installers write?" for a removal:
//!   receipt families (one product = many receipts), an exclusivity guard so
//!   nothing shared with surviving plugins is offered, an allowlist of safe
//!   Library roots, and subtree proof before offering a directory whole.
//! - [`remover`] moves everything to the Trash, batched by scope: one Trash
//!   call for user files, one privileged call — one admin prompt — for
//!   everything in `/Library`, no matter how much is selected.
//! - [`commands`] is the thin Tauri layer that wires these to the frontend.
//!
//! A bundle's `id` is its filesystem path — there is deliberately no other
//! identity. External effects (`pkgutil`, the Trash, the filesystem) sit
//! behind the [`receipts::PkgUtil`], [`remover::Trasher`] and [`reversal::Fs`]
//! traits, so every module's logic is tested without touching the real system.

mod commands;
mod error;
mod keywords;
mod model;
mod receipts;
mod remover;
mod reversal;
mod scanner;
mod search;
mod usage;
mod vst3meta;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(search::SearchIndex::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::plugin_details,
            commands::remove_items,
            commands::removal_preview,
            commands::reveal_in_finder,
            commands::save_export,
            commands::index_search,
            commands::semantic_search,
            commands::scan_usage,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
