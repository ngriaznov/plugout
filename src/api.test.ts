import { describe, expect, it } from "vitest";
import { isCanceled } from "./api";

describe("isCanceled", () => {
  it("recognizes a structured cancel", () => {
    expect(isCanceled({ kind: "canceled" })).toBe(true);
  });
  it("rejects other kinds and junk", () => {
    expect(isCanceled({ kind: "io", detail: "disk full" })).toBe(false);
    expect(isCanceled(new Error("boom"))).toBe(false);
    expect(isCanceled(undefined)).toBe(false);
    expect(isCanceled("canceled")).toBe(false);
  });
});
