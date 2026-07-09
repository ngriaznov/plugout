import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PluginBundle, PluginDetails, RemovalResult } from "./types";
import {
  mockStartScan,
  mockPluginDetails,
  mockRemoveItems,
  mockRevealInFinder,
  mockListen,
} from "./api.mock";

// Outside a Tauri window (plain `vite` in a browser) fall back to the mock
// backend so the frontend can be developed and tested without a Rust build.
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Commands
export const startScan = () => (isTauri ? invoke<void>("start_scan") : mockStartScan());
export const pluginDetails = (id: string) =>
  isTauri ? invoke<PluginDetails>("plugin_details", { id }) : mockPluginDetails(id);
export const removeItems = (ids: string[]) =>
  isTauri ? invoke<RemovalResult[]>("remove_items", { ids }) : mockRemoveItems(ids);
export const revealInFinder = (path: string) =>
  isTauri ? invoke<void>("reveal_in_finder", { path }) : mockRevealInFinder();

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
