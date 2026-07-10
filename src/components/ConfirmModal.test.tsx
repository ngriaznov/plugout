// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Format, PluginBundle, RemovalPreview, Scope } from "../types";
import { ConfirmModal } from "./ConfirmModal";
import { removalPreview } from "../api";

// The modal fetches its support-file preview from ../api on mount; mock it so
// tests stay hermetic and can drive loading vs. resolved states explicitly.
vi.mock("../api", () => ({
  removalPreview: vi.fn(),
}));

const mockRemovalPreview = vi.mocked(removalPreview);

const emptyPreview: RemovalPreview = { supportFiles: [], skippedShared: 0 };

afterEach(cleanup);

beforeEach(() => {
  mockRemovalPreview.mockReset();
  mockRemovalPreview.mockResolvedValue(emptyPreview);
});

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

function deferredPreview() {
  let resolve!: (p: RemovalPreview) => void;
  const promise = new Promise<RemovalPreview>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

function setup(
  bundles: PluginBundle[],
  opts: {
    allBundles?: PluginBundle[];
    onCancel?: () => void;
    onConfirm?: (extraPaths: string[]) => void;
    busy?: boolean;
  } = {},
) {
  const onCancel = opts.onCancel ?? vi.fn();
  const onConfirm = opts.onConfirm ?? vi.fn();
  const utils = render(
    <ConfirmModal
      bundles={bundles}
      allBundles={opts.allBundles ?? bundles}
      onCancel={onCancel}
      onConfirm={onConfirm}
      busy={opts.busy ?? false}
    />,
  );
  return { ...utils, onCancel, onConfirm };
}

describe("ConfirmModal grouping", () => {
  it("groups selected bundles per plugin, one item per plugin", async () => {
    const bundles = [
      mk({ id: "a", format: "AU", name: "Pigments" }),
      mk({ id: "b", format: "VST3", name: "Pigments" }),
      mk({ id: "c", name: "Serum", vendor: "Xfer" }),
    ];
    setup(bundles);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(2);
  });

  it("shows format badges of only the selected bundles and the per-plugin size", async () => {
    const bundles = [
      mk({ id: "a", format: "AU", name: "Pigments", sizeBytes: 100 }),
      mk({ id: "b", format: "VST3", name: "Pigments", sizeBytes: 200 }),
    ];
    setup(bundles);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    const item = screen.getByRole("listitem");
    expect(item).toHaveTextContent("AU");
    expect(item).toHaveTextContent("VST3");
    expect(item).not.toHaveTextContent("VST2");
    expect(item).toHaveTextContent("300 B");
  });
});

describe("ConfirmModal title", () => {
  it("says Move N items to Trash for N > 1", async () => {
    const bundles = [mk({ id: "a", name: "Pigments" }), mk({ id: "b", name: "Serum" })];
    setup(bundles);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(screen.getByRole("heading")).toHaveTextContent("Move 2 items to Trash?");
  });

  it("uses singular for N = 1", async () => {
    setup([mk({ id: "a" })]);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(screen.getByRole("heading")).toHaveTextContent("Move 1 item to Trash?");
  });
});

describe("ConfirmModal administrator warning", () => {
  it("is shown when a system-scope bundle is included", async () => {
    setup([mk({ id: "a", scope: "system" })]);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(
      screen.getByText("System items require an administrator password."),
    ).toBeInTheDocument();
  });

  it("is not shown when every bundle is user-scope", async () => {
    setup([mk({ id: "a", scope: "user" })]);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(screen.queryByText(/administrator password/)).not.toBeInTheDocument();
  });
});

describe("ConfirmModal actions", () => {
  it("calls onCancel when Cancel is clicked", async () => {
    const user = userEvent.setup();
    const { onCancel } = setup([mk({ id: "a" })]);
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onConfirm with the (empty) support paths when Move to Trash is clicked", async () => {
    const user = userEvent.setup();
    const { onConfirm } = setup([mk({ id: "a" })]);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Move to Trash" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith([]);
  });

  it("disables both buttons and shows Removing… when busy", async () => {
    setup([mk({ id: "a" })], { busy: true });
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    const confirmBtn = screen.getByRole("button", { name: "Removing…" });
    expect(confirmBtn).toBeDisabled();
  });
});

describe("ConfirmModal overlay", () => {
  it("calls onCancel when the overlay is clicked", async () => {
    const user = userEvent.setup();
    const { onCancel, container } = setup([mk({ id: "a" })]);
    await user.click(container.querySelector(".overlay")!);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("does not call onCancel when clicking inside the dialog", async () => {
    const user = userEvent.setup();
    const { onCancel } = setup([mk({ id: "a" })]);
    await user.click(screen.getByRole("dialog"));
    expect(onCancel).not.toHaveBeenCalled();
  });
});

describe("ConfirmModal support-file preview", () => {
  it("shows a pending status while the preview request is in flight", async () => {
    const { promise, resolve } = deferredPreview();
    mockRemovalPreview.mockReturnValueOnce(promise);
    setup([mk({ id: "a" })]);

    expect(screen.getByRole("status")).toHaveTextContent("Checking for support files…");

    resolve(emptyPreview);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
  });

  it("shows the support-item count/size with a checked-by-default toggle, and reveals paths on show", async () => {
    const preview: RemovalPreview = {
      supportFiles: [
        { path: "/Users/you/Library/Preferences/com.arturia.acid.plist", sizeBytes: 2048 },
        { path: "/Users/you/Library/Caches/com.arturia.acid", sizeBytes: 4096 },
      ],
      skippedShared: 0,
    };
    mockRemovalPreview.mockResolvedValueOnce(preview);
    const user = userEvent.setup();
    setup([mk({ id: "a" })]);

    const checkbox = await screen.findByRole("checkbox");
    expect(checkbox).toBeChecked();
    const toggleLabel = checkbox.closest("label")!;
    expect(toggleLabel).toHaveTextContent("Also remove 2 support items");
    expect(toggleLabel).toHaveTextContent("6.0 KB");

    expect(screen.queryByText(preview.supportFiles[0].path)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "show" }));
    expect(screen.getByText(preview.supportFiles[0].path, { exact: false })).toBeInTheDocument();
    expect(screen.getByText(preview.supportFiles[1].path, { exact: false })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "hide" }));
    expect(screen.queryByText(preview.supportFiles[0].path)).not.toBeInTheDocument();
  });

  it("passes the support file paths to onConfirm when the toggle stays on", async () => {
    const preview: RemovalPreview = {
      supportFiles: [{ path: "/Users/you/Library/Caches/com.arturia.acid", sizeBytes: 4096 }],
      skippedShared: 0,
    };
    mockRemovalPreview.mockResolvedValueOnce(preview);
    const user = userEvent.setup();
    const { onConfirm } = setup([mk({ id: "a" })]);

    await screen.findByRole("checkbox");
    await user.click(screen.getByRole("button", { name: "Move to Trash" }));
    expect(onConfirm).toHaveBeenCalledWith(["/Users/you/Library/Caches/com.arturia.acid"]);
  });

  it("passes an empty array to onConfirm when the toggle is switched off", async () => {
    const preview: RemovalPreview = {
      supportFiles: [{ path: "/Users/you/Library/Caches/com.arturia.acid", sizeBytes: 4096 }],
      skippedShared: 0,
    };
    mockRemovalPreview.mockResolvedValueOnce(preview);
    const user = userEvent.setup();
    const { onConfirm } = setup([mk({ id: "a" })]);

    await user.click(await screen.findByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Move to Trash" }));
    expect(onConfirm).toHaveBeenCalledWith([]);
  });

  it("shows a note when support files were kept because they're shared with plugins staying installed", async () => {
    const preview: RemovalPreview = { supportFiles: [], skippedShared: 2 };
    mockRemovalPreview.mockResolvedValueOnce(preview);
    setup([mk({ id: "a" })]);

    expect(
      await screen.findByText(/2 installer packages are shared with plugins staying installed/),
    ).toBeInTheDocument();
  });

  it("renders no support section when there are no support files and nothing was skipped", async () => {
    mockRemovalPreview.mockResolvedValueOnce(emptyPreview);
    setup([mk({ id: "a" })]);

    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.queryByText(/support/i)).not.toBeInTheDocument();
  });
});
