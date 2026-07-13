// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ExportModal } from "./ExportModal";

afterEach(cleanup);

describe("ExportModal", () => {
  it("offers selected-only and everything-shown", async () => {
    const onChoose = vi.fn();
    render(<ExportModal count={42} selectedCount={3} onChoose={onChoose} onCancel={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: /selected only/i }));
    expect(onChoose).toHaveBeenCalledWith("selected");
  });
  it("cancel calls onCancel", async () => {
    const onCancel = vi.fn();
    render(<ExportModal count={42} selectedCount={3} onChoose={() => {}} onCancel={onCancel} />);
    await userEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });
});
