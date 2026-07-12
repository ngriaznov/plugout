import { describe, it, expect } from "vitest";
import { formatBytes, mergePlugins, sortPlugins, compareVersions, gateHits, matchUsage } from "./util";
import type { PluginBundle, Format, Scope } from "./types";

describe("formatBytes", () => {
  it("formats zero", () => expect(formatBytes(0)).toBe("0 B"));
  it("formats KB", () => expect(formatBytes(2048)).toBe("2.0 KB"));
  it("formats MB", () => expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB"));
  it("formats GB", () => expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB"));
});

function mk(over: Partial<PluginBundle> & { id: string }): PluginBundle {
  return {
    name: "Acid V",
    vendor: "Arturia",
    version: "1.0.0",
    format: "VST3" as Format,
    bundleId: "com.arturia.acid",
    path: `/Library/${over.id}`,
    sizeBytes: 10,
    scope: "system" as Scope,
    packageId: null,
    category: null,
    copyright: null,
    ...over,
  };
}

describe("mergePlugins", () => {
  it("merges bundles sharing vendor+name into one plugin", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU" }),
      mk({ id: "b", format: "VST3" }),
      mk({ id: "c", format: "VST2" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].name).toBe("Acid V");
    expect(plugins[0].installs).toHaveLength(3);
  });

  it("keeps distinct plugins apart, including same name from different vendors", () => {
    const plugins = mergePlugins([
      mk({ id: "a" }),
      mk({ id: "b", name: "Pigments", bundleId: "com.arturia.pigments" }),
      mk({ id: "c", name: "Pigments", vendor: "Other Co", bundleId: "com.other.pigments" }),
    ]);
    expect(plugins).toHaveLength(3);
  });

  it("merges installs whose vendor spellings differ only by case", () => {
    // AU derives the vendor from AudioComponents ("discoDSP"); other formats
    // fall back to a bundle-id segment ("discodsp").
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", name: "Discovery Pro", vendor: "discoDSP", bundleId: "" }),
      mk({ id: "b", format: "VST3", name: "Discovery Pro", vendor: "discodsp", bundleId: "" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].installs).toHaveLength(2);
  });

  it("merges installs sharing bundle id and name even when vendor wording differs", () => {
    const plugins = mergePlugins([
      mk({
        id: "a",
        format: "AU",
        name: "Drumazon2",
        vendor: "D16 Group Audio Software",
        bundleId: "com.d16group.Drumazon2",
      }),
      mk({ id: "b", format: "VST3", name: "Drumazon2", vendor: "d16group", bundleId: "com.d16group.Drumazon2" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].vendor).toBe("D16 Group Audio Software");
  });

  it("keeps same-name plugins from different vendors apart when bundle ids differ", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Pigments", vendor: "Arturia", bundleId: "com.arturia.pigments" }),
      mk({ id: "b", name: "Pigments", vendor: "Other Co", bundleId: "com.other.pigments" }),
    ]);
    expect(plugins).toHaveLength(2);
  });

  it("keeps same vendor+name merged when bundle ids differ per format", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", name: "Diva", vendor: "u-he", bundleId: "com.u-he.Diva.au" }),
      mk({ id: "b", format: "VST3", name: "Diva", vendor: "u-he", bundleId: "com.u-he.Diva.vst3" }),
    ]);
    expect(plugins).toHaveLength(1);
  });

  it("never unions on empty bundle ids", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Alpha", vendor: "One", bundleId: "" }),
      mk({ id: "b", name: "Alpha", vendor: "Two", bundleId: "" }),
    ]);
    expect(plugins).toHaveLength(2);
  });

  it("sorts installs in canonical format order (AU, VST3, VST2, CLAP, AAX)", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AAX" }),
      mk({ id: "b", format: "AU" }),
      mk({ id: "c", format: "VST2" }),
      mk({ id: "d", format: "VST3" }),
    ]);
    expect(plugins[0].installs.map((i) => i.format)).toEqual(["AU", "VST3", "VST2", "AAX"]);
  });

  it("sums sizes and unions scopes", () => {
    const plugins = mergePlugins([
      mk({ id: "a", sizeBytes: 100, scope: "system" }),
      mk({ id: "b", sizeBytes: 50, scope: "user", format: "VST2" }),
    ]);
    expect(plugins[0].sizeBytes).toBe(150);
    expect(plugins[0].scopes).toEqual(["user", "system"]);
  });

  it("prefers a dotted version from a non-AU install over AU's integer build", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", version: "65797" }),
      mk({ id: "b", format: "VST3", version: "1.1.5.6367" }),
    ]);
    expect(plugins[0].version).toBe("1.1.5.6367");
  });

  it("falls back to any dotted version, then any non-empty version", () => {
    const onlyAuDotted = mergePlugins([mk({ id: "a", format: "AU", version: "2.3.1" })]);
    expect(onlyAuDotted[0].version).toBe("2.3.1");
    const onlyInt = mergePlugins([mk({ id: "a", format: "AU", version: "65797" })]);
    expect(onlyInt[0].version).toBe("65797");
    const empty = mergePlugins([mk({ id: "a", version: "" })]);
    expect(empty[0].version).toBe("");
  });

  it("sorts by name case-insensitively, then vendor, by default", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "zebra" }),
      mk({ id: "b", name: "Analog Lab V" }),
      mk({ id: "c", name: "Pigments", vendor: "Zeta", bundleId: "com.zeta.pigments" }),
      mk({ id: "d", name: "Pigments", vendor: "Alpha", bundleId: "com.alpha.pigments" }),
    ]);
    expect(plugins.map((p) => `${p.name}/${p.vendor}`)).toEqual([
      "Analog Lab V/Arturia",
      "Pigments/Alpha",
      "Pigments/Zeta",
      "zebra/Arturia",
    ]);
  });

  it("takes category and copyright from the first install that carries them", () => {
    const [p] = mergePlugins([
      mk({ id: "a", format: "VST3", category: null, copyright: null }),
      mk({ id: "b", format: "AU", category: "instrument", copyright: "© Xfer" }),
      mk({ id: "c", format: "VST2", category: "effect", copyright: "© other" }),
    ]);
    expect(p.category).toBe("instrument");
    expect(p.copyright).toBe("© Xfer");
  });

  it("leaves category and copyright null when no install carries them", () => {
    const [p] = mergePlugins([mk({ id: "a" })]);
    expect(p.category).toBeNull();
    expect(p.copyright).toBeNull();
  });

  it("merges vendor spelling variants (sumu: 'Madrona Labs' vs 'madronalabs')", () => {
    const plugins = mergePlugins([
      mk({ id: "au", format: "AU", name: "sumu", vendor: "Madrona Labs",
           bundleId: "com.madronalabs.vst3plugin.sumu.audiounit" }),
      mk({ id: "v3", format: "VST3", name: "Sumu", vendor: "madronalabs",
           bundleId: "com.madronalabs.vst3.sumu" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].installs).toHaveLength(2);
  });

  it("merges punctuation and spacing name variants", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", name: "TAL-Reverb-4", vendor: "TAL Software" }),
      mk({ id: "b", format: "VST3", name: "TAL Reverb 4", vendor: "TAL Software" }),
      mk({ id: "c", format: "AU", name: "Serum 2", vendor: "Xfer Records", bundleId: "com.xfer.serum2" }),
      mk({ id: "d", format: "VST3", name: "Serum2", vendor: "Xfer Records", bundleId: "com.xfer.serum2" }),
    ]);
    expect(plugins).toHaveLength(2);
  });

  it("absorbs family variants into the base product (VCV Rack 2)", () => {
    const plugins = mergePlugins([
      mk({ id: "au", format: "AU", name: "VCV Rack 2", vendor: "VCV", bundleId: "com.vcvrack.rack" }),
      mk({ id: "fx", format: "AU", name: "VCV Rack 2 FX", vendor: "VCV", bundleId: "com.vcvrack.rack" }),
      mk({ id: "v3", format: "VST3", name: "VCV Rack 2", vendor: "vcvrack", bundleId: "com.vcvrack.rack" }),
      mk({ id: "cl", format: "CLAP", name: "VCV Rack 2", vendor: "vcvrack", bundleId: "com.vcvrack.rack" }),
      mk({ id: "app", format: "APP", name: "VCV Rack 2 Pro", vendor: "vcvrack", bundleId: "com.vcvrack.rackpro" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].name).toBe("VCV Rack 2");
    expect(plugins[0].installs).toHaveLength(5);
  });

  it("absorbs suffix-word companions (Serum FX) but not digit siblings (Serum 2)", () => {
    const plugins = mergePlugins([
      mk({ id: "s", name: "Serum", vendor: "Xfer Records", bundleId: "com.xferrecords.serum" }),
      mk({ id: "sfx", name: "Serum FX", vendor: "Xfer Records", bundleId: "com.xferrecords.serumfx" }),
      mk({ id: "s2", name: "Serum 2", vendor: "Xfer Records", bundleId: "com.xferrecords.serum2" }),
      mk({ id: "s2fx", name: "Serum 2 FX", vendor: "Xfer Records", bundleId: "com.xferrecords.serum2fx" }),
    ]);
    expect(plugins).toHaveLength(2);
    const names = plugins.map((p) => p.name).sort();
    expect(names).toEqual(["Serum", "Serum 2"]);
  });

  it("absorbs vendor-prefixed names (FabFilter Pro-Q 3) but not digit siblings", () => {
    const plugins = mergePlugins([
      mk({ id: "q3", name: "Pro-Q 3", vendor: "FabFilter", bundleId: "com.fabfilter.proq3" }),
      mk({ id: "q3f", name: "FabFilter Pro-Q 3", vendor: "FabFilter", bundleId: "com.fabfilter.proq3.full" }),
      mk({ id: "q4", name: "Pro-Q 4", vendor: "FabFilter", bundleId: "com.fabfilter.proq4" }),
    ]);
    expect(plugins).toHaveLength(2);
  });

  it("does not merge sibling products whose names only overlap (Ozone, Kontakt)", () => {
    const plugins = mergePlugins([
      mk({ id: "oe", name: "Ozone 11 Equalizer", vendor: "iZotope", bundleId: "com.izotope.ozone11eq" }),
      mk({ id: "oi", name: "Ozone 11 Imager", vendor: "iZotope", bundleId: "com.izotope.ozone11img" }),
      mk({ id: "k7", name: "Kontakt 7", vendor: "Native Instruments", bundleId: "com.ni.kontakt7" }),
      mk({ id: "k8", name: "Kontakt 8", vendor: "Native Instruments", bundleId: "com.ni.kontakt8" }),
    ]);
    expect(plugins).toHaveLength(4);
  });

  it("ignores format markers when comparing family names", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Kontakt 7", vendor: "Native Instruments", bundleId: "com.ni.kontakt7" }),
      mk({ id: "b", name: "Kontakt 7 (VST3)", vendor: "Native Instruments", bundleId: "com.ni.kontakt7.vst3" }),
    ]);
    expect(plugins).toHaveLength(1);
    expect(plugins[0].name).toBe("Kontakt 7");
  });

  it("bridges vendor spellings via shared bundle id only within one call", () => {
    const au = mk({ id: "au", format: "AU", name: "Decimort 2", vendor: "D16 Group Audio Software", bundleId: "com.d16group.decimort2" });
    const v3 = mk({ id: "v3", format: "VST3", name: "Decimort 2", vendor: "d16group", bundleId: "com.d16group.decimort2" });
    expect(mergePlugins([au, v3])).toHaveLength(1); // together: bridged
    expect(mergePlugins([au])[0].key).not.toBe(mergePlugins([v3])[0].key); // apart: different keys
  });

  it("does not treat a ccTLD second-level domain as a shared org", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Claro", vendor: "Sonnox", bundleId: "uk.co.sonnox.claro" }),
      mk({ id: "b", name: "Claro Pro", vendor: "oeksound", bundleId: "uk.co.oeksound.claropro" }),
    ]);
    expect(plugins).toHaveLength(2);
  });
});

describe("compareVersions", () => {
  it("compares numeric segments, not strings", () => {
    expect(compareVersions("1.10.0", "1.2.0")).toBeGreaterThan(0);
    expect(compareVersions("1.1.5", "1.1.5")).toBe(0);
    expect(compareVersions("5.2", "5.12.3.6637")).toBeLessThan(0);
  });
  it("treats missing segments as zero and empty as lowest", () => {
    expect(compareVersions("1.1", "1.1.0")).toBe(0);
    expect(compareVersions("", "0.1")).toBeLessThan(0);
  });
});

describe("gateHits", () => {
  const hit = (id: string, score: number) => ({ id, score });
  it("keeps hits within GATE_RATIO of the best and drops the tail", () => {
    const kept = gateHits([hit("a", 0.44), hit("b", 0.41), hit("c", 0.30)]);
    expect(kept.map((h) => h.id)).toEqual(["a", "b"]); // 0.30 < 0.7 * 0.44
  });
  it("keeps a lone weak hit (floor already applied backend-side)", () => {
    expect(gateHits([hit("a", 0.26)])).toHaveLength(1);
  });
  it("caps at GATE_CAP", () => {
    const many = Array.from({ length: 12 }, (_, i) => hit(`p${i}`, 0.5));
    expect(gateHits(many)).toHaveLength(8);
  });
  it("returns empty for empty input", () => {
    expect(gateHits([])).toEqual([]);
  });
});

describe("sortPlugins", () => {
  const plugins = mergePlugins([
    mk({ id: "a", name: "Zebra", vendor: "u-he", sizeBytes: 10, version: "2.9.3" }),
    mk({ id: "b", name: "Acid V", vendor: "Arturia", sizeBytes: 50, version: "1.10.0" }),
    mk({ id: "c", name: "Acid V", vendor: "Arturia", sizeBytes: 25, version: "1.10.0", format: "AU" }),
    mk({ id: "d", name: "Serum", vendor: "Xfer Records", sizeBytes: 30, version: "1.2.0" }),
  ]);

  it("sorts by vendor with name as tiebreaker", () => {
    expect(sortPlugins(plugins, "vendor", 1).map((p) => p.vendor)).toEqual([
      "Arturia", "u-he", "Xfer Records",
    ]);
  });
  it("sorts by size descending", () => {
    expect(sortPlugins(plugins, "size", -1).map((p) => p.sizeBytes)).toEqual([75, 30, 10]);
  });
  it("sorts by version numerically", () => {
    expect(sortPlugins(plugins, "version", 1).map((p) => p.version)).toEqual([
      "1.2.0", "1.10.0", "2.9.3",
    ]);
  });
  it("sorts by format count", () => {
    expect(sortPlugins(plugins, "formats", -1)[0].name).toBe("Acid V");
  });
  it("does not mutate its input", () => {
    const before = plugins.map((p) => p.key);
    sortPlugins(plugins, "size", -1);
    expect(plugins.map((p) => p.key)).toEqual(before);
  });
});

describe("matchUsage", () => {
  const hit = (name: string, vendor: string, project: string, mtimeMs: number) =>
    ({ name, vendor, project, mtimeMs });

  it("matches by folded name with containment vendors, counting distinct projects", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Ozone 12 Vintage Limiter", vendor: "iZotope, Inc.", bundleId: "com.izotope.vl12" }),
    ]);
    const usage = matchUsage(plugins, [
      hit("Ozone 12 Vintage Limiter", "iZotope", "/p/one.RPP", 100),
      hit("Ozone 12 Vintage Limiter", "iZotope", "/p/two.RPP", 300),
      hit("Ozone 12 Vintage Limiter", "iZotope", "/p/two.RPP", 300), // same project twice
    ]);
    expect(usage.get(plugins[0].key)).toEqual({ projects: 2, lastUsedMs: 300, lastProject: "/p/two.RPP" });
  });

  it("matches vendor-less hits by name alone and family variants by token subset", () => {
    const plugins = mergePlugins([
      mk({ id: "t", name: "TAL-Reverb-4", vendor: "TAL Software", bundleId: "com.tal.reverb4" }),
    ]);
    const usage = matchUsage(plugins, [hit("TAL Reverb 4 Plugin", "", "/p/x.als", 50)]);
    expect(usage.get(plugins[0].key)?.projects).toBe(1);
  });

  it("does not match different digits or unrelated names", () => {
    const plugins = mergePlugins([
      mk({ id: "q3", name: "Pro-Q 3", vendor: "FabFilter", bundleId: "com.ff.q3" }),
    ]);
    const usage = matchUsage(plugins, [
      hit("Pro-Q 4", "FabFilter", "/p/a.RPP", 1),
      hit("Serum", "Xfer Records", "/p/a.RPP", 1),
    ]);
    expect(usage.size).toBe(0);
  });
});

describe("sortPlugins by usage", () => {
  it("orders by lastUsedMs desc and keeps unseen plugins last in both directions", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: "Alpha", vendor: "V", bundleId: "com.v.a" }),
      mk({ id: "b", name: "Beta", vendor: "V", bundleId: "com.v.b" }),
      mk({ id: "c", name: "Gamma", vendor: "V", bundleId: "com.v.c" }),
    ]);
    const usage = new Map([
      [plugins.find((p) => p.name === "Alpha")!.key, { projects: 1, lastUsedMs: 100, lastProject: "/1" }],
      [plugins.find((p) => p.name === "Gamma")!.key, { projects: 2, lastUsedMs: 900, lastProject: "/2" }],
    ]);
    const desc = sortPlugins(plugins, "used", -1, usage).map((p) => p.name);
    expect(desc).toEqual(["Gamma", "Alpha", "Beta"]);
    const asc = sortPlugins(plugins, "used", 1, usage).map((p) => p.name);
    expect(asc).toEqual(["Alpha", "Gamma", "Beta"]); // unseen still last
  });
});
