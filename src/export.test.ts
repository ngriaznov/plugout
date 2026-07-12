import { describe, it, expect } from "vitest";
import { exportCsv, exportJson } from "./export";
import { mergePlugins } from "./util";
import type { Format, PluginBundle, Scope } from "./types";

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

describe("exportCsv", () => {
  it("emits a header and one row per install", () => {
    const plugins = mergePlugins([mk({ id: "a", format: "AU" }), mk({ id: "b" })]);
    const lines = exportCsv(plugins).trimEnd().split("\n");
    expect(lines[0]).toBe(
      "product,name,vendor,version,format,scope,category,sizeBytes,path,bundleId,packageId",
    );
    expect(lines).toHaveLength(3);
    expect(lines[1]).toContain("AU"); // installs in FORMATS order
  });

  it("quotes fields containing commas and doubles embedded quotes", () => {
    const plugins = mergePlugins([
      mk({ id: "a", name: 'Big, "Bad" Delay', vendor: "ACME, Inc" }),
    ]);
    const row = exportCsv(plugins).trimEnd().split("\n")[1];
    expect(row).toContain('"Big, ""Bad"" Delay"');
    expect(row).toContain('"ACME, Inc"');
  });

  it("handles an empty list (header only)", () => {
    expect(exportCsv([]).trimEnd()).toBe(
      "product,name,vendor,version,format,scope,category,sizeBytes,path,bundleId,packageId",
    );
  });
});

describe("exportJson", () => {
  it("nests installs under products", () => {
    const plugins = mergePlugins([mk({ id: "a", format: "AU" }), mk({ id: "b" })]);
    const parsed = JSON.parse(exportJson(plugins));
    expect(parsed).toHaveLength(1);
    expect(parsed[0].name).toBe("Acid V");
    expect(parsed[0].installs).toHaveLength(2);
    expect(parsed[0].installs[0].format).toBe("AU");
    expect(parsed[0].installs[0].path).toBe("/Library/a");
  });

  it("serializes an empty list as []", () => {
    expect(JSON.parse(exportJson([]))).toEqual([]);
  });
});
