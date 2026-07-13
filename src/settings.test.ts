// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from "vitest";
import { getSettings, setSettings } from "./settings";

beforeEach(() => localStorage.clear());

describe("settings", () => {
  it("defaults usageScan to false", () => {
    expect(getSettings()).toEqual({ usageScan: false, extraScanDirs: [] });
  });

  it("survives malformed storage", () => {
    localStorage.setItem("plugout:settings", "{not json");
    expect(getSettings()).toEqual({ usageScan: false, extraScanDirs: [] });
  });

  it("persists a patch and merges over defaults", () => {
    expect(setSettings({ usageScan: true })).toEqual({ usageScan: true, extraScanDirs: [] });
    expect(getSettings()).toEqual({ usageScan: true, extraScanDirs: [] });
  });

  it("ignores unknown keys in storage", () => {
    localStorage.setItem("plugout:settings", '{"usageScan":true,"bogus":1}');
    expect(getSettings()).toEqual({ usageScan: true, extraScanDirs: [] });
  });

  it("defaults extraScanDirs to empty and sanitizes junk", () => {
    localStorage.setItem("plugout:settings", JSON.stringify({ extraScanDirs: ["/a", 5, null] }));
    expect(getSettings().extraScanDirs).toEqual(["/a"]);
    localStorage.setItem("plugout:settings", JSON.stringify({ extraScanDirs: "nope" }));
    expect(getSettings().extraScanDirs).toEqual([]);
  });

  it("persists extraScanDirs through setSettings", () => {
    setSettings({ extraScanDirs: ["/x", "/y"] });
    expect(getSettings().extraScanDirs).toEqual(["/x", "/y"]);
  });
});
