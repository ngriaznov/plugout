// Browser-only mock of the Tauri backend so the frontend can be developed and
// visually tested with plain `vite` (no Rust build). api.ts delegates here when
// the app runs outside a Tauri window.
import type { Format, PluginBundle, PluginDetails, RemovalPreview, RemovalResult, Scope } from "./types";
import type { ReceiptUpdate } from "./api";

type Def = [name: string, vendor: string, version: string, mb: number, formats: Format[], scope?: Scope, pkg?: string | null];

const DEFS: Def[] = [
  ["Acid V", "Arturia", "1.1.5.6367", 55, ["AU", "VST3", "VST2"], "system", "com.arturia.acidv"],
  ["Analog Lab V", "Arturia", "5.12.3.6637", 99, ["AU", "VST3", "VST2"], "system", "com.arturia.analoglab"],
  ["ARP 2600 V3", "Arturia", "3.13.4.6366", 54, ["AU", "VST3", "VST2"], "system", "com.arturia.arp2600"],
  ["Augmented BRASS", "Arturia", "2.0.2.6369", 57, ["AU", "VST3", "VST2"], "system", "com.arturia.augbrass"],
  ["Augmented GRAND PIANO", "Arturia", "2.0.2.6369", 57, ["AU", "VST3", "VST2"], "system", "com.arturia.augpiano"],
  ["Pigments", "Arturia", "5.0.1.4310", 312, ["AU", "VST3", "CLAP"], "system", "com.arturia.pigments"],
  ["Serum", "Xfer Records", "1.365", 248, ["AU", "VST3", "VST2", "APP"], "user", null],
  ["Arturia Software Center", "Arturia", "2.6.1", 210, ["APP"], "system", "com.arturia.asc"],
  ["ValhallaRoom", "Valhalla DSP", "2.1.1", 18, ["AU", "VST3", "AAX"], "user", null],
  ["Ozone 11 Elements", "iZotope", "11.0.2", 421, ["AU", "VST3", "AAX"], "system", "com.izotope.ozone11"],
  ["Vital", "Vital Audio", "1.5.5", 176, ["AU", "VST3", "CLAP"], "user", null],
  ["Decapitator", "Soundtoys", "5.5.4", 44, ["AU", "VST3", "VST2", "AAX"], "system", "com.soundtoys.all"],
  ["Pro-Q 3", "FabFilter", "3.24", 31, ["AU", "VST3", "VST2", "AAX"], "system", "com.fabfilter.proq3"],
];

const AU_BUILDS: Record<string, string> = { "Acid V": "65797", "Analog Lab V": "330755", "ARP 2600 V3": "199940" };

function bundlesFor([name, vendor, version, mb, formats, scope = "system", pkg = null]: Def): PluginBundle[] {
  return formats.map((format) => ({
    id: `${vendor}-${name}-${format}`.replace(/\s+/g, "-").toLowerCase(),
    name,
    vendor,
    version: format === "AU" ? AU_BUILDS[name] ?? version : version,
    format,
    bundleId: `com.${vendor.replace(/\s+/g, "").toLowerCase()}.${name.replace(/\s+/g, "").toLowerCase()}`,
    path:
      format === "APP"
        ? `/Applications/${name}.app`
        : (scope === "user" ? "/Users/you" : "") +
          `/Library/Audio/Plug-Ins/${format === "AU" ? "Components" : format.startsWith("VST") ? format : format}/${name}.${
            format === "AU" ? "component" : format === "VST2" ? "vst" : format.toLowerCase()
          }`,
    sizeBytes: Math.round(mb * 1024 * 1024 * (format === "AU" ? 1 : 0.98)),
    scope,
    packageId: pkg,
  }));
}

const ALL = DEFS.flatMap(bundlesFor);
const trashed = new Set<string>();
const live = () => ALL.filter((b) => !trashed.has(b.id));

type Listeners = {
  batch: ((b: PluginBundle[]) => void)[];
  done: ((n: number) => void)[];
  receipt: ((u: ReceiptUpdate[]) => void)[];
};
const listeners: Listeners = { batch: [], done: [], receipt: [] };

export const mockStartScan = async (): Promise<void> => {
  const bundles = live();
  const mid = Math.ceil(bundles.length / 2);
  setTimeout(() => listeners.batch.forEach((cb) => cb(bundles.slice(0, mid))), 500);
  setTimeout(() => listeners.batch.forEach((cb) => cb(bundles.slice(mid))), 1000);
  setTimeout(() => listeners.done.forEach((cb) => cb(bundles.length)), 1200);
};

export const mockPluginDetails = async (id: string): Promise<PluginDetails> => {
  await new Promise((r) => setTimeout(r, 400)); // simulate pkgutil latency
  const b = ALL.find((x) => x.id === id);
  return {
    packageId: b?.packageId ?? null,
    filesToTrash: b
      ? [b.path, `/Users/you/Library/Preferences/${b.bundleId}.plist`, `/Users/you/Library/Caches/${b.bundleId}`]
      : [],
  };
};

export const mockRemoveItems = async (ids: string[]): Promise<RemovalResult[]> => {
  await new Promise((r) => setTimeout(r, 600));
  return ids.map((id) => {
    trashed.add(id);
    const b = ALL.find((x) => x.id === id);
    return { id, path: b?.path ?? id, status: "trashed" as const, message: null };
  });
};

export const mockRevealInFinder = async (): Promise<void> => {};

export const mockRemovalPreview = async (removing: string[]): Promise<RemovalPreview> => {
  await new Promise((r) => setTimeout(r, 350));
  const removed = ALL.filter((b) => removing.includes(b.id));
  const names = [...new Set(removed.map((b) => b.name))];
  // Arturia packages are "shared" unless every Arturia bundle is being removed —
  // demonstrates the exclusivity guard in browser dev mode.
  const arturia = removed.some((b) => b.vendor === "Arturia");
  const allArturiaRemoved = ALL.filter(
    (b) => b.vendor === "Arturia" && !trashed.has(b.id),
  ).every((b) => removing.includes(b.id));
  return {
    supportFiles: names.flatMap((n) => [
      { path: `/Library/Application Support/${n}`, sizeBytes: 38_000_000 },
      { path: `/Users/you/Library/Preferences/com.vendor.${n.replace(/\s+/g, "").toLowerCase()}.plist`, sizeBytes: 4_096 },
    ]),
    skippedShared: arturia && !allArturiaRemoved ? 1 : 0,
  };
};

export const mockListen = <T>(event: string, cb: (payload: T) => void): Promise<() => void> => {
  const pool =
    event === "scan:batch" ? listeners.batch : event === "scan:done" ? listeners.done : listeners.receipt;
  pool.push(cb as never);
  return Promise.resolve(() => {
    const i = pool.indexOf(cb as never);
    if (i >= 0) pool.splice(i, 1);
  });
};
