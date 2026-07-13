// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { render, screen, within, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { ComponentType } from "react";

afterEach(cleanup);
beforeEach(() => localStorage.clear());

// theme.ts reads window.matchMedia at module scope; jsdom doesn't implement
// it. App must be imported dynamically, after the stub is in place, since a
// static `import App from "./App"` would evaluate theme.ts too early.
let App: ComponentType;

beforeAll(async () => {
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as typeof window.matchMedia;
  ({ default: App } = await import("./App"));
});

// The mock backend streams scan batches on real setTimeouts (500ms/1000ms)
// and flips loading off at 1200ms, and a removal round-trip takes 600ms;
// give async finds room past the default 1000ms testing-library timeout.
const LONG = { timeout: 3000 };

// The header's "Select all plugins" checkbox lives in a <tr> too, so picking
// the first row with *any* checkbox would grab it instead of a plugin row.
function firstPluginRow(): HTMLElement {
  const row = screen
    .getAllByRole("row")
    .find((r) => {
      const checkbox = within(r).queryByRole("checkbox");
      return checkbox && checkbox.getAttribute("aria-label") !== "Select all plugins";
    });
  if (!row) throw new Error("no plugin row with a per-row checkbox found");
  return row;
}

describe("App integration (mock backend)", () => {
  it("scans and renders plugin rows", async () => {
    render(<App />);
    expect(await screen.findAllByRole("row")).not.toHaveLength(0);
    // Count line reads "N plugins" only once the scan settles (loading: false).
    expect(await screen.findByText(/plugins$/, undefined, LONG)).toBeInTheDocument();
    expect(screen.getAllByRole("row").length).toBeGreaterThan(1);
  });

  it("selects a plugin and removes it through the confirm flow", async () => {
    render(<App />);
    await screen.findByText(/plugins$/, undefined, LONG);

    await userEvent.click(within(firstPluginRow()).getByRole("checkbox"));
    await userEvent.click(await screen.findByRole("button", { name: /remove/i }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /move to trash/i }));

    expect(await screen.findByText(/moved to Trash/, undefined, LONG)).toBeInTheDocument();
  });

  it("export with a selection opens the choice modal", async () => {
    render(<App />);
    await screen.findByText(/plugins$/, undefined, LONG);

    await userEvent.click(within(firstPluginRow()).getByRole("checkbox"));
    await userEvent.click(screen.getByRole("button", { name: "Export" }));

    expect(await screen.findByRole("dialog", { name: /export/i })).toBeInTheDocument();
  });

  it("search narrows the table to the empty state", async () => {
    render(<App />);
    await screen.findByText(/plugins$/, undefined, LONG);

    const search = screen.getByLabelText("Search plugins");
    // Stays under 3 chars so useSemanticRelated's debounce never fires and
    // can't repopulate the table with "Related matches" rows mid-assertion.
    await userEvent.type(search, "zz");

    expect(await screen.findByText(/no plugins match/i)).toBeInTheDocument();
  });

  it("slash focuses search", async () => {
    render(<App />);
    await screen.findByText(/plugins$/, undefined, LONG);

    await userEvent.keyboard("/");

    expect(document.activeElement).toBe(screen.getByLabelText("Search plugins"));
  });
});
