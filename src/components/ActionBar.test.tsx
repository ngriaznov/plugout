// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ActionBar } from "./ActionBar";

afterEach(cleanup);

describe("ActionBar counts", () => {
  it("shows the plugin count, pluralized", () => {
    render(<ActionBar pluginCount={1} installCount={1} sizeBytes={0} onClear={vi.fn()} onRemove={vi.fn()} />);
    expect(screen.getByText("1 plugin")).toBeInTheDocument();
  });

  it("pluralizes for more than one plugin", () => {
    render(<ActionBar pluginCount={3} installCount={3} sizeBytes={0} onClear={vi.fn()} onRemove={vi.fn()} />);
    expect(screen.getByText("3 plugins")).toBeInTheDocument();
  });

  it("shows install count only when it differs from plugin count", () => {
    render(<ActionBar pluginCount={2} installCount={5} sizeBytes={0} onClear={vi.fn()} onRemove={vi.fn()} />);
    expect(screen.getByText(/5 installs/)).toBeInTheDocument();
  });

  it("omits install count when it matches plugin count", () => {
    render(<ActionBar pluginCount={2} installCount={2} sizeBytes={0} onClear={vi.fn()} onRemove={vi.fn()} />);
    expect(screen.queryByText(/installs/)).not.toBeInTheDocument();
  });

  it("shows the formatted reclaimable size", () => {
    render(
      <ActionBar pluginCount={1} installCount={1} sizeBytes={5 * 1024 * 1024} onClear={vi.fn()} onRemove={vi.fn()} />,
    );
    expect(screen.getByText(/5\.0 MB reclaimable/)).toBeInTheDocument();
  });
});

describe("ActionBar actions", () => {
  it("calls onClear when Clear is clicked", async () => {
    const user = userEvent.setup();
    const onClear = vi.fn();
    render(<ActionBar pluginCount={1} installCount={1} sizeBytes={0} onClear={onClear} onRemove={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it("calls onRemove when Remove… is clicked", async () => {
    const user = userEvent.setup();
    const onRemove = vi.fn();
    render(<ActionBar pluginCount={1} installCount={1} sizeBytes={0} onClear={vi.fn()} onRemove={onRemove} />);
    await user.click(screen.getByRole("button", { name: "Remove…" }));
    expect(onRemove).toHaveBeenCalledTimes(1);
  });
});
