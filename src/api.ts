import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PluginBundle, PluginDetails, RemovalPreview, RemovalResult } from "./types";
import {
  mockStartScan,
  mockPluginDetails,
  mockRemovalPreview,
  mockRemoveItems,
  mockRevealInFinder,
  mockListen,
  mockIndexSearch,
  mockSemanticSearch,
  mockSaveExport,
  mockScanUsage,
} from "./api.mock";

// Outside a Tauri window (plain `vite` in a browser) fall back to the mock
// backend so the frontend can be developed and tested without a Rust build.
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Structured backend error, serialized from Rust's CmdError. */
export interface CmdError {
  kind: "canceled" | "notFound" | "permissionDenied" | "io" | "internal";
  detail?: string;
}

export const isCanceled = (e: unknown): boolean =>
  typeof e === "object" && e !== null && (e as CmdError).kind === "canceled";

// Commands
export const startScan = (extraDirs: string[] = []) =>
  isTauri ? invoke<void>("start_scan", { extraDirs }) : mockStartScan();
export const pluginDetails = (id: string) =>
  isTauri ? invoke<PluginDetails>("plugin_details", { id }) : mockPluginDetails(id);
export const removeItems = (ids: string[]) =>
  isTauri ? invoke<RemovalResult[]>("remove_items", { ids }) : mockRemoveItems(ids);
export const revealInFinder = (path: string) =>
  isTauri ? invoke<void>("reveal_in_finder", { path }) : mockRevealInFinder();
/** Support files a removal would additionally clean up. `bundles` must be the
 * full scanned set — exclusivity is judged against everything installed. */
export const removalPreview = (removing: string[], bundles: PluginBundle[]) => {
  const owned = bundles.map((b) => ({ id: b.id, packageId: b.packageId }));
  return isTauri
    ? invoke<RemovalPreview>("removal_preview", { removing, bundles: owned })
    : mockRemovalPreview(removing);
};

// Semantic search (vendored ternlight model in the Rust backend)
export interface SearchDoc {
  id: string;
  name: string;
  vendor: string;
  category: string;
}
export interface SearchHit {
  id: string;
  score: number;
}
export const indexSearch = (docs: SearchDoc[]) =>
  isTauri ? invoke<void>("index_search", { docs }) : mockIndexSearch();
export const semanticSearch = (query: string) =>
  isTauri ? invoke<SearchHit[]>("semantic_search", { query }) : mockSemanticSearch();

// Scan events (emitted by start_scan, off the main thread)
export interface ReceiptUpdate {
  id: string;
  packageId: string | null;
}

const on = <T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> =>
  isTauri ? listen<T>(event, (e) => cb(e.payload)) : mockListen<T>(event, cb);

export const onScanBatch = (cb: (batch: PluginBundle[]) => void): Promise<UnlistenFn> =>
  on<PluginBundle[]>("scan:batch", cb);

export const onScanDone = (cb: (total: number) => void): Promise<UnlistenFn> =>
  on<number>("scan:done", cb);

export const onReceiptUpdate = (cb: (updates: ReceiptUpdate[]) => void): Promise<UnlistenFn> =>
  on<ReceiptUpdate[]>("receipt:update", cb);

export const onEnrichDone = (cb: () => void): Promise<UnlistenFn> =>
  on<null>("enrich:done", cb);

export interface ExportFile {
  name: string;
  contents: string;
}
export const saveExport = (files: ExportFile[]) =>
  isTauri ? invoke<string>("save_export", { files }) : mockSaveExport();

// DAW project usage (plugins referenced by REAPER/Ableton/Studio One/Logic Pro projects on disk)
export interface UsageHit {
  name: string;
  vendor: string;
  project: string;
  mtimeMs: number;
}
export const scanUsage = (knownNames: string[]) =>
  isTauri ? invoke<UsageHit[]>("scan_usage", { knownNames }) : mockScanUsage();
