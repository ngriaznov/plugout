import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// Mirrors the guard in api.ts: outside a Tauri window there is no updater.
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type UpdateState =
  | { phase: "idle" }
  | { phase: "available"; version: string }
  | { phase: "downloading"; version: string; percent: number | null }
  | { phase: "ready"; version: string }
  | { phase: "error"; message: string };

let pending: Update | null = null;

/// Returns the available update's version, or null when up to date (or not in Tauri).
export async function checkForUpdate(): Promise<string | null> {
  if (!isTauri) return null;
  const update = await check();
  if (!update) return null;
  pending = update;
  return update.version;
}

/// Downloads and installs the pending update, reporting progress as 0–100 (or
/// null when the server sends no content length). Call `restartApp` afterwards.
export async function downloadAndInstall(onProgress: (percent: number | null) => void): Promise<void> {
  if (!pending) throw new Error("no update pending");
  let total: number | null = null;
  let received = 0;
  await pending.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? null;
        onProgress(total ? 0 : null);
        break;
      case "Progress":
        received += event.data.chunkLength;
        onProgress(total ? Math.min(100, Math.round((received / total) * 100)) : null);
        break;
      case "Finished":
        onProgress(100);
        break;
    }
  });
}

export const restartApp = (): Promise<void> => relaunch();
