// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsModal } from "./SettingsModal";
import type { ComponentProps } from "react";

afterEach(cleanup);

function renderModal(overrides: Partial<ComponentProps<typeof SettingsModal>> = {}) {
  const props: ComponentProps<typeof SettingsModal> = {
    settings: { usageScan: false, extraScanDirs: [] },
    onChange: vi.fn(),
    themePref: "system",
    onTheme: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
  render(<SettingsModal {...props} />);
  return props;
}

describe("SettingsModal", () => {
  it("reflects the setting and reports toggles as a patch", async () => {
    const { onChange } = renderModal();
    const box = screen.getByRole("checkbox", { name: /Scan DAW projects/ });
    expect(box).not.toBeChecked();
    await userEvent.click(box);
    expect(onChange).toHaveBeenCalledWith({ usageScan: true });
  });

  it("closes via the close button", async () => {
    const { onClose } = renderModal({ settings: { usageScan: true, extraScanDirs: [] } });
    await userEvent.click(screen.getByRole("button", { name: /close settings/i }));
    expect(onClose).toHaveBeenCalled();
  });

  it("shows the current theme and reports a change", async () => {
    const { onTheme } = renderModal({ themePref: "light" });
    expect(screen.getByRole("radio", { name: "Light" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Dark" })).toHaveAttribute("aria-checked", "false");
    await userEvent.click(screen.getByRole("radio", { name: "Dark" }));
    expect(onTheme).toHaveBeenCalledWith("dark");
  });

  it("adds a folder via the text input fallback", async () => {
    const { onChange } = renderModal();
    await userEvent.type(screen.getByLabelText("Folder path"), "/a/plugins");
    await userEvent.click(screen.getByRole("button", { name: /add/i }));
    expect(onChange).toHaveBeenCalledWith({ extraScanDirs: ["/a/plugins"] });
  });

  it("removes a folder via its remove button", async () => {
    const { onChange } = renderModal({
      settings: { usageScan: false, extraScanDirs: ["/a", "/b"] },
    });
    await userEvent.click(screen.getByRole("button", { name: "Remove /a" }));
    expect(onChange).toHaveBeenCalledWith({ extraScanDirs: ["/b"] });
  });

  it("ignores duplicate folders", async () => {
    const { onChange } = renderModal({
      settings: { usageScan: false, extraScanDirs: ["/a"] },
    });
    await userEvent.type(screen.getByLabelText("Folder path"), "/a");
    await userEvent.click(screen.getByRole("button", { name: /add/i }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
