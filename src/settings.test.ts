// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from "vitest";
import { getSettings, setSettings } from "./settings";

beforeEach(() => localStorage.clear());

describe("settings", () => {
  it("defaults usageScan to false", () => {
    expect(getSettings()).toEqual({ usageScan: false });
  });

  it("survives malformed storage", () => {
    localStorage.setItem("plugout:settings", "{not json");
    expect(getSettings()).toEqual({ usageScan: false });
  });

  it("persists a patch and merges over defaults", () => {
    expect(setSettings({ usageScan: true })).toEqual({ usageScan: true });
    expect(getSettings()).toEqual({ usageScan: true });
  });

  it("ignores unknown keys in storage", () => {
    localStorage.setItem("plugout:settings", '{"usageScan":true,"bogus":1}');
    expect(getSettings()).toEqual({ usageScan: true });
  });
});
