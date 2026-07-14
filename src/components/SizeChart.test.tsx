// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { SizeChart } from "./SizeChart";
import type { PluginBundle } from "../types";

afterEach(cleanup);

const bundle = (format: PluginBundle["format"], sizeBytes: number): PluginBundle => ({
  id: `/p/${format}-${sizeBytes}`,
  name: "X",
  vendor: "V",
  version: "1.0",
  format,
  bundleId: "com.v.x",
  path: `/p/${format}-${sizeBytes}`,
  sizeBytes,
  scope: "user",
  packageId: null,
  category: null,
  copyright: null,
});

describe("SizeChart", () => {
  it("renders one legend row per present format with its size", () => {
    render(<SizeChart bundles={[bundle("VST3", 2048), bundle("AU", 1024), bundle("VST3", 1024)]} />);
    const rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    // fixed format order: AU before VST3, regardless of size rank
    expect(rows[0]).toHaveTextContent("AU");
    expect(rows[1]).toHaveTextContent("VST3");
    expect(rows[1]).toHaveTextContent("3.0 KB");
  });

  it("labels the bar with the total and each segment with format, size and share", () => {
    render(<SizeChart bundles={[bundle("AU", 3072), bundle("CLAP", 1024)]} />);
    expect(screen.getByRole("img", { name: /Disk use by format, 4\.0 KB total/ })).toBeInTheDocument();
    expect(screen.getByTitle("AU — 3.0 KB (75%)")).toBeInTheDocument();
    expect(screen.getByTitle("CLAP — 1.0 KB (25%)")).toBeInTheDocument();
  });

  it("renders nothing while there is no size data", () => {
    const { container } = render(<SizeChart bundles={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
