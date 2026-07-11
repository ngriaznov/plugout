import type { Plugin, PluginBundle, Scope } from "./types";
import { FORMATS } from "./types";

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  if (i === 0) return `${bytes} B`;
  return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
}

const DOTTED = /^\d+(\.\d+)+/;

// AU reports an integer build number (e.g. "65797") where other formats carry a
// human version, so prefer a dotted version from a non-AU install.
function displayVersion(installs: PluginBundle[]): string {
  const nonAuDotted = installs.find((b) => b.format !== "AU" && DOTTED.test(b.version));
  if (nonAuDotted) return nonAuDotted.version;
  const dotted = installs.find((b) => DOTTED.test(b.version));
  if (dotted) return dotted.version;
  return installs.find((b) => b.version !== "")?.version ?? "";
}

const SCOPE_ORDER: Scope[] = ["user", "system"];

// Format markers carry no product identity ("Kontakt 7 (VST3)" is Kontakt 7)
// and must not contribute digits to the version-sibling check.
const FORMAT_TOKENS = new Set(["vst", "vst2", "vst3", "au", "audiounit", "aax", "clap", "app"]);

// Alphanumeric runs minus format markers, split into letter/digit subtokens:
// "Kontakt 7 (VST3)" → ["kontakt", "7"]; "Serum2" → ["serum", "2"].
const tokensOf = (name: string): string[] =>
  (name.toLowerCase().match(/[a-z0-9]+/g) ?? [])
    .filter((run) => !FORMAT_TOKENS.has(run))
    .flatMap((run) => run.match(/[a-z]+|\d+/g) ?? []);

// Digit changes mean a different product (Serum vs Serum 2, Pro-Q 3 vs 4).
const digitsOf = (tokens: string[]): string =>
  tokens.filter((t) => /^\d+$/.test(t)).join(".");

// The reverse-DNS org ("com.vcvrack") bridges a family whose members carry
// different bundle ids and vendor spellings across formats.
const orgOf = (bundleId: string): string | null => {
  const seg = bundleId.toLowerCase().split(".");
  const n = seg[0]?.length === 2 ? 3 : 2; // ccTLD reverse-DNS: org is the third segment
  return seg.length >= n ? seg.slice(0, n).join(".") : null;
};

// Shortest name (fewest tokens, then fewest characters) is the base product
// name of a family — "VCV Rack 2", not "VCV Rack 2 MIDI FX".
const canonicalName = (list: PluginBundle[]): string => {
  let best = list[0].name;
  for (const b of list) {
    const [n, bn] = [tokensOf(b.name).length, tokensOf(best).length];
    if (n < bn || (n === bn && b.name.length < best.length)) best = b.name;
  }
  return best;
};

// The same plugin carries two vendor spellings: AU bundles report the human
// vendor name from AudioComponents ("D16 Group Audio Software") while other
// formats fall back to a bundle-id segment ("d16group"). Installs therefore
// merge when they agree on either identity — case-folded vendor+name, or
// bundle id+name (the id is spelling-independent and shared across formats).
// Vendor spellings that differ only in case/punctuation ("Madrona Labs" vs
// "madronalabs") fold together.
// Union-find, because a middle install can bridge two others transitively.
export function mergePlugins(bundles: PluginBundle[]): Plugin[] {
  // Identity fold: case- and punctuation-insensitive, so "Madrona Labs" ≡
  // "madronalabs" and "Serum 2" ≡ "Serum2". Keys need an explicit separator
  // because folding eats the spaces that used to provide one.
  const fold = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
  const parent = new Map<string, string>();
  const find = (k: string): string => {
    const p = parent.get(k) ?? k;
    if (p === k) return k;
    const root = find(p);
    parent.set(k, root);
    return root;
  };
  const union = (a: string, b: string) => {
    const ra = find(a);
    const rb = find(b);
    if (ra !== rb) parent.set(rb, ra);
  };
  const keysOf = (b: PluginBundle): string[] => {
    const name = fold(b.name);
    const keys = [`vn:${fold(b.vendor)}|${name}`];
    if (b.bundleId) keys.push(`id:${b.bundleId.toLowerCase()}|${name}`);
    return keys;
  };
  for (const b of bundles) {
    const [first, ...rest] = keysOf(b);
    for (const k of rest) union(first, k);
  }
  const map = new Map<string, PluginBundle[]>();
  for (const b of bundles) {
    const key = find(keysOf(b)[0]);
    const list = map.get(key);
    if (list) list.push(b);
    else map.set(key, [b]);
  }

  // Family absorption: "Serum FX", "VCV Rack 2 Pro" belong to the product
  // whose name theirs extends. Merge group EXT into group BASE when they
  // share an identity (vendor, bundle id, or reverse-DNS org), their digit
  // sequences match (digit changes are different products), and BASE's name
  // tokens are a subset of EXT's. Equal sets are allowed — "Kontakt 7" and
  // "Kontakt 7 (VST3)" tokenize identically once format markers drop.
  interface Fam { key: string; tokens: Set<string>; digits: string; identity: Set<string> }
  const fams: Fam[] = [...map.entries()].map(([key, list]) => {
    const tokens = new Set(tokensOf(canonicalName(list)));
    const identity = new Set<string>();
    for (const b of list) {
      identity.add(`v:${fold(b.vendor)}`);
      if (b.bundleId) {
        identity.add(`b:${b.bundleId.toLowerCase()}`);
        const org = orgOf(b.bundleId);
        if (org) identity.add(`o:${org}`);
      }
    }
    return { key, tokens: tokens, digits: digitsOf([...tokens]), identity };
  });
  for (const base of fams) {
    for (const ext of fams) {
      if (base === ext || base.digits !== ext.digits) continue;
      if (base.tokens.size > ext.tokens.size) continue;
      if (![...base.tokens].every((t) => ext.tokens.has(t))) continue;
      if (![...base.identity].some((i) => ext.identity.has(i))) continue;
      union(base.key, ext.key);
    }
  }
  const groups = new Map<string, PluginBundle[]>();
  for (const [key, list] of map) {
    const root = find(key);
    const existing = groups.get(root);
    if (existing) existing.push(...list);
    else groups.set(root, [...list]);
  }

  const plugins: Plugin[] = [...groups.entries()].map(([key, list]) => {
    const installs = [...list].sort(
      (a, b) => FORMATS.indexOf(a.format) - FORMATS.indexOf(b.format),
    );
    const scopes = new Set(installs.map((b) => b.scope));
    return {
      key,
      name: canonicalName(installs),
      vendor: installs[0].vendor,
      version: displayVersion(installs),
      installs,
      sizeBytes: installs.reduce((n, b) => n + b.sizeBytes, 0),
      scopes: SCOPE_ORDER.filter((s) => scopes.has(s)),
      // AU carries category; any install may carry copyright — take the first present.
      category: installs.find((b) => b.category)?.category ?? null,
      copyright: installs.find((b) => b.copyright)?.copyright ?? null,
    };
  });
  return plugins.sort(byName);
}

const byName = (a: Plugin, b: Plugin) =>
  a.name.toLowerCase().localeCompare(b.name.toLowerCase()) ||
  a.vendor.toLowerCase().localeCompare(b.vendor.toLowerCase());

// Numeric segment-wise comparison ("1.10.0" > "1.2.0"); missing segments are 0.
export function compareVersions(a: string, b: string): number {
  const seg = (v: string) => (v.match(/\d+/g) ?? []).map(Number);
  const sa = seg(a);
  const sb = seg(b);
  for (let i = 0; i < Math.max(sa.length, sb.length); i++) {
    const d = (sa[i] ?? 0) - (sb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

export type SortKey = "name" | "vendor" | "formats" | "version" | "size";
export type SortDir = 1 | -1;

const COMPARATORS: Record<SortKey, (a: Plugin, b: Plugin) => number> = {
  name: byName,
  vendor: (a, b) => a.vendor.toLowerCase().localeCompare(b.vendor.toLowerCase()),
  formats: (a, b) => a.installs.length - b.installs.length,
  version: (a, b) => compareVersions(a.version, b.version),
  size: (a, b) => a.sizeBytes - b.sizeBytes,
};

export function sortPlugins(plugins: Plugin[], key: SortKey, dir: SortDir): Plugin[] {
  const cmp = COMPARATORS[key];
  return [...plugins].sort((a, b) => dir * cmp(a, b) || byName(a, b));
}

// Semantic hits are gated relative to the strongest hit that survived
// substring exclusion: a weak best hit tightens nothing (the floor already
// ran in Rust), but a strong one drowns out barely-related tail matches.
export const GATE_RATIO = 0.7;
export const GATE_CAP = 8;

/** Semantic hits within GATE_RATIO of the best surviving score, capped at GATE_CAP.
 * `hits` must be sorted descending by score (the backend returns them sorted). */
export function gateHits<T extends { score: number }>(hits: T[]): T[] {
  if (hits.length === 0) return [];
  const gate = hits[0].score * GATE_RATIO;
  return hits.filter((h) => h.score >= gate).slice(0, GATE_CAP);
}
