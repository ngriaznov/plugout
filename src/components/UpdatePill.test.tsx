// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { UpdatePill } from "./UpdatePill";

afterEach(cleanup);

const noop = () => {};

describe("UpdatePill", () => {
  it("renders nothing while idle", () => {
    const { container } = render(
      <UpdatePill state={{ phase: "idle" }} onDownload={noop} onRestart={noop} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("offers the available version and starts the download on click", async () => {
    const onDownload = vi.fn();
    render(
      <UpdatePill
        state={{ phase: "available", version: "0.2.0" }}
        onDownload={onDownload}
        onRestart={noop}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "v0.2.0 available" }));
    expect(onDownload).toHaveBeenCalledOnce();
  });

  it("shows indeterminate then percent progress while downloading", () => {
    const { rerender } = render(
      <UpdatePill
        state={{ phase: "downloading", version: "0.2.0", percent: null }}
        onDownload={noop}
        onRestart={noop}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Downloading…");
    rerender(
      <UpdatePill
        state={{ phase: "downloading", version: "0.2.0", percent: 42 }}
        onDownload={noop}
        onRestart={noop}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Downloading… 42%");
  });

  it("restarts on click once ready", async () => {
    const onRestart = vi.fn();
    render(
      <UpdatePill
        state={{ phase: "ready", version: "0.2.0" }}
        onDownload={noop}
        onRestart={onRestart}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Restart to update" }));
    expect(onRestart).toHaveBeenCalledOnce();
  });

  it("shows the failure with the message as tooltip", () => {
    render(
      <UpdatePill
        state={{ phase: "error", message: "boom" }}
        onDownload={noop}
        onRestart={noop}
      />,
    );
    expect(screen.getByText("Update failed")).toHaveAttribute("title", "boom");
  });
});
