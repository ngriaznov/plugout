// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, within, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Format, PluginBundle, Scope } from "../types";
import { mergePlugins, type SortDir, type SortKey } from "../util";
import { PluginList } from "./PluginList";

vi.mock("../detailsCache", () => ({ prefetchDetails: vi.fn() }));

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

function baseProps() {
  return {
    plugins: [],
    selected: new Set<string>(),
    loading: false,
    query: "",
    sort: { key: "name" as SortKey, dir: 1 as SortDir },
    onSort: vi.fn(),
    onTogglePlugin: vi.fn(),
    onToggleInstall: vi.fn(),
    onToggleAll: vi.fn(),
    onRowClick: vi.fn(),
    onClearSearch: vi.fn(),
  };
}

describe("PluginList rows", () => {
  it("renders one row per plugin with name, vendor, chips, and formatted size", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", name: "Pigments", vendor: "Arturia", sizeBytes: 1024 }),
      mk({ id: "b", format: "VST3", name: "Pigments", vendor: "Arturia", sizeBytes: 1024 }),
      mk({ id: "c", name: "Serum", vendor: "Xfer", sizeBytes: 2048 }),
    ]);
    render(<PluginList {...baseProps()} plugins={plugins} />);

    const rows = screen.getAllByRole("row").slice(1);
    expect(rows).toHaveLength(2);

    const pigmentsRow = within(rows[0]);
    expect(pigmentsRow.getByText("Pigments")).toBeInTheDocument();
    expect(pigmentsRow.getAllByText("Arturia").length).toBeGreaterThan(0);
    expect(pigmentsRow.getByRole("button", { name: "AU" })).toBeInTheDocument();
    expect(pigmentsRow.getByRole("button", { name: "VST3" })).toBeInTheDocument();
    expect(pigmentsRow.getByText("2.0 KB")).toBeInTheDocument();
  });
});

describe("PluginList header checkbox", () => {
  it("is unchecked when nothing is selected", () => {
    const plugins = mergePlugins([mk({ id: "a" }), mk({ id: "b", name: "Serum" })]);
    render(<PluginList {...baseProps()} plugins={plugins} selected={new Set()} />);
    const header = screen.getByRole("checkbox", { name: "Select all plugins" });
    expect(header).not.toBeChecked();
    expect(header).toHaveProperty("indeterminate", false);
  });

  it("is indeterminate when some installs are selected", () => {
    const plugins = mergePlugins([mk({ id: "a" }), mk({ id: "b", name: "Serum" })]);
    render(<PluginList {...baseProps()} plugins={plugins} selected={new Set(["a"])} />);
    const header = screen.getByRole("checkbox", { name: "Select all plugins" });
    expect(header).not.toBeChecked();
    expect(header).toHaveProperty("indeterminate", true);
  });

  it("is checked when every install is selected", () => {
    const plugins = mergePlugins([mk({ id: "a" }), mk({ id: "b", name: "Serum" })]);
    render(<PluginList {...baseProps()} plugins={plugins} selected={new Set(["a", "b"])} />);
    const header = screen.getByRole("checkbox", { name: "Select all plugins" });
    expect(header).toBeChecked();
  });

  it("calls onToggleAll when clicked", async () => {
    const user = userEvent.setup();
    const props = baseProps();
    const plugins = mergePlugins([mk({ id: "a" })]);
    render(<PluginList {...props} plugins={plugins} />);
    await user.click(screen.getByRole("checkbox", { name: "Select all plugins" }));
    expect(props.onToggleAll).toHaveBeenCalledTimes(1);
  });
});

describe("PluginList row checkbox", () => {
  it("is indeterminate when some but not all installs of a plugin are selected", () => {
    const plugins = mergePlugins([
      mk({ id: "a", format: "AU", name: "Pigments" }),
      mk({ id: "b", format: "VST3", name: "Pigments" }),
    ]);
    render(<PluginList {...baseProps()} plugins={plugins} selected={new Set(["a"])} />);
    const row = screen.getByRole("checkbox", { name: "Select Pigments" });
    expect(row).not.toBeChecked();
    expect(row).toHaveProperty("indeterminate", true);
  });

  it("calls onTogglePlugin with the plugin when clicked", async () => {
    const user = userEvent.setup();
    const props = baseProps();
    const plugins = mergePlugins([mk({ id: "a", name: "Pigments" })]);
    render(<PluginList {...props} plugins={plugins} />);
    await user.click(screen.getByRole("checkbox", { name: "Select Pigments" }));
    expect(props.onTogglePlugin).toHaveBeenCalledTimes(1);
    expect(props.onTogglePlugin).toHaveBeenCalledWith(plugins[0]);
  });
});

describe("PluginList format chips", () => {
  it("clicking a chip calls onToggleInstall with the bundle id and not onRowClick", async () => {
    const user = userEvent.setup();
    const props = baseProps();
    const plugins = mergePlugins([mk({ id: "a", format: "AU", name: "Pigments" })]);
    render(<PluginList {...props} plugins={plugins} />);
    await user.click(screen.getByRole("button", { name: "AU" }));
    expect(props.onToggleInstall).toHaveBeenCalledWith("a");
    expect(props.onRowClick).not.toHaveBeenCalled();
  });
});

describe("PluginList row click", () => {
  it("calls onRowClick with the plugin", async () => {
    const user = userEvent.setup();
    const props = baseProps();
    const plugins = mergePlugins([mk({ id: "a", name: "Pigments" })]);
    render(<PluginList {...props} plugins={plugins} />);
    await user.click(screen.getByText("Pigments"));
    expect(props.onRowClick).toHaveBeenCalledWith(plugins[0]);
  });
});

describe("PluginList sort headers", () => {
  it.each([
    ["Plugin", "name"],
    ["Vendor", "vendor"],
    ["Formats", "formats"],
    ["Version", "version"],
    ["Size", "size"],
  ] satisfies [string, SortKey][])("clicking %s calls onSort with %s", async (label, key) => {
    const user = userEvent.setup();
    const props = baseProps();
    render(<PluginList {...props} plugins={[]} />);
    await user.click(screen.getByRole("button", { name: new RegExp(`^${label}`) }));
    expect(props.onSort).toHaveBeenCalledWith(key);
  });

  it("exposes aria-sort only on the active header", () => {
    render(<PluginList {...baseProps()} plugins={[]} sort={{ key: "vendor", dir: -1 }} />);
    expect(screen.getByRole("columnheader", { name: /Vendor/ })).toHaveAttribute(
      "aria-sort",
      "descending",
    );
    expect(screen.getByRole("columnheader", { name: /Plugin/ })).not.toHaveAttribute("aria-sort");
  });
});

describe("PluginList empty state", () => {
  it("shows the query and a Clear search button wired to onClearSearch", async () => {
    const user = userEvent.setup();
    const props = baseProps();
    render(<PluginList {...props} plugins={[]} query="serum" />);
    expect(screen.getByText(/No plugins match/)).toHaveTextContent("serum");
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(props.onClearSearch).toHaveBeenCalledTimes(1);
  });

  it("renders skeleton rows instead of the empty state while loading with no plugins", () => {
    const { container } = render(<PluginList {...baseProps()} plugins={[]} loading query="" />);
    expect(container.querySelectorAll(".skel-row")).toHaveLength(9);
    expect(screen.queryByText("No plugins found")).not.toBeInTheDocument();
  });
});

describe("related results", () => {
  it("renders a divider and related rows when related plugins are present", () => {
    const related = mergePlugins([mk({ id: "r1", name: "ValhallaVintageVerb", vendor: "Valhalla DSP" })]);
    render(<PluginList {...baseProps()} plugins={mergePlugins([mk({ id: "a" })])} related={related} />);
    expect(screen.getByText("Related matches")).toBeInTheDocument();
    expect(screen.getByText("ValhallaVintageVerb")).toBeInTheDocument();
  });

  it("renders no divider when there are no related plugins", () => {
    render(<PluginList {...baseProps()} plugins={mergePlugins([mk({ id: "a" })])} />);
    expect(screen.queryByText("Related matches")).not.toBeInTheDocument();
  });

  it("keeps the empty state only when both lists are empty", () => {
    const related = mergePlugins([mk({ id: "r1", name: "ValhallaVintageVerb", vendor: "Valhalla DSP" })]);
    render(<PluginList {...baseProps()} query="reverb" related={related} />);
    expect(screen.queryByText(/No plugins match/)).not.toBeInTheDocument();
  });

  it("hides related plugins whose product already appears in the main list", () => {
    // Same product, two vendor spellings that fold to the same identity
    // (case/punctuation only): merged plugins share the same key.
    const plugins = mergePlugins([mk({ id: "a", name: "Decimort 2", vendor: "d16group", bundleId: "com.d16group.decimort2" })]);
    const related = mergePlugins([mk({ id: "b", name: "Decimort 2", vendor: "D16 Group", bundleId: "com.d16group.decimort2" })]);
    expect(related[0].key).toBe(plugins[0].key); // precondition: identical product key
    render(<PluginList {...baseProps()} plugins={plugins} related={related} />);
    expect(screen.queryByText("Related matches")).not.toBeInTheDocument();
    expect(screen.getAllByText("Decimort 2")).toHaveLength(1);
  });
});
