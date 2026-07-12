// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsModal } from "./SettingsModal";

afterEach(cleanup);

describe("SettingsModal", () => {
  it("reflects the setting and reports toggles as a patch", async () => {
    const onChange = vi.fn();
    render(
      <SettingsModal settings={{ usageScan: false }} onChange={onChange} onClose={vi.fn()} />,
    );
    const box = screen.getByRole("checkbox", { name: /Scan DAW projects/ });
    expect(box).not.toBeChecked();
    await userEvent.click(box);
    expect(onChange).toHaveBeenCalledWith({ usageScan: true });
  });

  it("closes via the close button", async () => {
    const onClose = vi.fn();
    render(
      <SettingsModal settings={{ usageScan: true }} onChange={vi.fn()} onClose={onClose} />,
    );
    await userEvent.click(screen.getByRole("button", { name: /close settings/i }));
    expect(onClose).toHaveBeenCalled();
  });
});
