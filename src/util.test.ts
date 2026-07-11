import { describe, it, expect } from "vitest";
import { formatBytes, mergePlugins, sortPlugins, compareVersions } from "./util";
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
