// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Format, PluginBundle, Scope } from "../types";
import { ConfirmModal } from "./ConfirmModal";

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
    scope: "user" as Scope,
    packageId: null,
    ...over,
  };
}

describe("ConfirmModal grouping", () => {
  it("groups selected bundles per plugin, one item per plugin", () => {
    const bundles = [
      mk({ id: "a", format: "AU", name: "Pigments" }),
      mk({ id: "b", format: "VST3", name: "Pigments" }),
      mk({ id: "c", name: "Serum", vendor: "Xfer" }),
    ];
    render(<ConfirmModal bundles={bundles} onCancel={vi.fn()} onConfirm={vi.fn()} busy={false} />);
    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(2);
  });

  it("shows format badges of only the selected bundles and the per-plugin size", () => {
    const bundles = [
      mk({ id: "a", format: "AU", name: "Pigments", sizeBytes: 100 }),
      mk({ id: "b", format: "VST3", name: "Pigments", sizeBytes: 200 }),
    ];
    render(<ConfirmModal bundles={bundles} onCancel={vi.fn()} onConfirm={vi.fn()} busy={false} />);
    const item = screen.getByRole("listitem");
    expect(item).toHaveTextContent("AU");
    expect(item).toHaveTextContent("VST3");
    expect(item).not.toHaveTextContent("VST2");
    expect(item).toHaveTextContent("300 B");
  });
});

describe("ConfirmModal title", () => {
  it("says Move N plugins to Trash for N > 1", () => {
    const bundles = [mk({ id: "a", name: "Pigments" }), mk({ id: "b", name: "Serum" })];
    render(<ConfirmModal bundles={bundles} onCancel={vi.fn()} onConfirm={vi.fn()} busy={false} />);
    expect(screen.getByRole("heading")).toHaveTextContent("Move 2 plugins to Trash?");
  });

  it("uses singular for N = 1", () => {
    const bundles = [mk({ id: "a" })];
    render(<ConfirmModal bundles={bundles} onCancel={vi.fn()} onConfirm={vi.fn()} busy={false} />);
    expect(screen.getByRole("heading")).toHaveTextContent("Move 1 plugin to Trash?");
  });
});

describe("ConfirmModal administrator warning", () => {
  it("is shown when a system-scope bundle is included", () => {
    const bundles = [mk({ id: "a", scope: "system" })];
    render(<ConfirmModal bundles={bundles} onCancel={vi.fn()} onConfirm={vi.fn()} busy={false} />);
    expect(screen.getByText(/administrator password/)).toBeInTheDocument();
  });

  it("is not shown when every bundle is user-scope", () => {
    const bundles = [mk({ id: "a", scope: "user" })];
    render(<ConfirmModal bundles={bundles} onCancel={vi.fn()} onConfirm={vi.fn()} busy={false} />);
    expect(screen.queryByText(/administrator password/)).not.toBeInTheDocument();
  });
});

describe("ConfirmModal actions", () => {
  it("calls onCancel when Cancel is clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<ConfirmModal bundles={[mk({ id: "a" })]} onCancel={onCancel} onConfirm={vi.fn()} busy={false} />);
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onConfirm when Move to Trash is clicked", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<ConfirmModal bundles={[mk({ id: "a" })]} onCancel={vi.fn()} onConfirm={onConfirm} busy={false} />);
    await user.click(screen.getByRole("button", { name: "Move to Trash" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("disables both buttons and shows Removing… when busy", () => {
    render(<ConfirmModal bundles={[mk({ id: "a" })]} onCancel={vi.fn()} onConfirm={vi.fn()} busy />);
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    const confirmBtn = screen.getByRole("button", { name: "Removing…" });
    expect(confirmBtn).toBeDisabled();
  });
});

describe("ConfirmModal overlay", () => {
  it("calls onCancel when the overlay is clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    const { container } = render(
      <ConfirmModal bundles={[mk({ id: "a" })]} onCancel={onCancel} onConfirm={vi.fn()} busy={false} />,
    );
    await user.click(container.querySelector(".overlay")!);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("does not call onCancel when clicking inside the dialog", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<ConfirmModal bundles={[mk({ id: "a" })]} onCancel={onCancel} onConfirm={vi.fn()} busy={false} />);
    await user.click(screen.getByRole("dialog"));
    expect(onCancel).not.toHaveBeenCalled();
  });
});
