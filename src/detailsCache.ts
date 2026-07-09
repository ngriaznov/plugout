import type { Plugin, PluginDetails } from "./types";
import { pluginDetails } from "./api";

// Session cache of plugin_details results, keyed by bundle id. Rows prefetch on
// hover, so by the time the inspector opens the data is usually already here.
const cache = new Map<string, Promise<PluginDetails>>();

export function getDetails(id: string): Promise<PluginDetails> {
  let p = cache.get(id);
  if (!p) {
    p = pluginDetails(id);
    p.catch(() => cache.delete(id)); // don't cache failures
    cache.set(id, p);
  }
  return p;
}

export function prefetchDetails(plugin: Plugin): void {
  for (const b of plugin.installs) {
    getDetails(b.id).catch(() => {});
  }
}

export function clearDetailsCache(): void {
  cache.clear();
}
