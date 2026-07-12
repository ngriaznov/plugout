// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Format, PluginBundle, Scope } from "../types";
import { mergePlugins } from "../util";
import { Inspector } from "./Inspector";

vi.mock("../detailsCache", () => ({
  getDetails: vi.fn(() => new Promise(() => {})), // never resolves — cards show skeletons
}));
vi.mock("../api", () => ({ revealInFinder: vi.fn() }));

afterEach(cleanup);

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

describe("Inspector install selection", () => {
  it("shows a checkbox per install reflecting the selection set", () => {
    const plugin = mergePlugins([mk({ id: "au1", format: "AU" }), mk({ id: "v31", format: "VST3" })])[0];
    render(
      <Inspector
        plugin={plugin}
        usage={null}
        selected={new Set(["au1"])}
        onToggleInstall={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const boxes = screen.getAllByRole("checkbox");
    expect(boxes).toHaveLength(2);
    expect(boxes[0]).toBeChecked(); // AU sorts first (FORMATS order)
    expect(boxes[1]).not.toBeChecked();
  });

  it("reports toggles with the install's bundle id", async () => {
    const onToggle = vi.fn();
    const plugin = mergePlugins([mk({ id: "au1", format: "AU" }), mk({ id: "v31", format: "VST3" })])[0];
    render(
      <Inspector plugin={plugin} usage={null} selected={new Set()} onToggleInstall={onToggle} onClose={vi.fn()} />,
    );
    await userEvent.click(screen.getAllByRole("checkbox")[1]);
    expect(onToggle).toHaveBeenCalledWith("v31");
  });

  it("labels checkboxes distinctly for same-format installs in different scopes", () => {
    const plugin = mergePlugins([
      mk({ id: "au-sys", format: "AU", scope: "system" }),
      mk({ id: "au-usr", format: "AU", scope: "user" }),
    ])[0];
    render(
      <Inspector plugin={plugin} usage={null} selected={new Set()} onToggleInstall={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.getByLabelText("Select user AU install")).toBeInTheDocument();
    expect(screen.getByLabelText("Select system AU install")).toBeInTheDocument();
  });
});

describe("usage line", () => {
  it("shows count and last-used date with a reveal affordance", () => {
    const plugin = mergePlugins([mk({ id: "a" })])[0];
    render(
      <Inspector
        plugin={plugin}
        usage={{ projects: 3, lastUsedMs: Date.UTC(2026, 0, 23), lastProject: "/p/AD System Demo.RPP" }}
        selected={new Set()}
        onToggleInstall={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText(/Used in 3 projects/)).toBeInTheDocument();
    expect(screen.getByText(/2026-01-23/)).toBeInTheDocument();
  });

  it("states the negative honestly when usage is null", () => {
    const plugin = mergePlugins([mk({ id: "a" })])[0];
    render(
      <Inspector plugin={plugin} usage={null} selected={new Set()} onToggleInstall={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.getByText(/Not seen in any DAW project/)).toBeInTheDocument();
  });
});
