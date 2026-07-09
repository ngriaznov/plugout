// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FormatBadge, FormatChip, FORMAT_COLORS } from "./FormatBadge";

afterEach(cleanup);

describe("FormatChip", () => {
  it("reflects selected state via aria-pressed and chip-on class", () => {
    render(<FormatChip format="AU" selected onToggle={vi.fn()} />);
    const chip = screen.getByRole("button", { name: "AU" });
    expect(chip).toHaveAttribute("aria-pressed", "true");
    expect(chip).toHaveClass("chip-on");
    expect(chip).toHaveAttribute("title", "Deselect AU");
  });

  it("reflects unselected state", () => {
    render(<FormatChip format="AU" selected={false} onToggle={vi.fn()} />);
    const chip = screen.getByRole("button", { name: "AU" });
    expect(chip).toHaveAttribute("aria-pressed", "false");
    expect(chip).not.toHaveClass("chip-on");
    expect(chip).toHaveAttribute("title", "Select AU");
  });

  it("calls onToggle when clicked", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    render(<FormatChip format="VST3" selected={false} onToggle={onToggle} />);
    await user.click(screen.getByRole("button"));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("exposes the --c custom property matching the format color", () => {
    render(<FormatChip format="CLAP" selected={false} onToggle={vi.fn()} />);
    const chip = screen.getByRole("button");
    expect(chip.style.getPropertyValue("--c")).toBe(FORMAT_COLORS.CLAP);
  });
});

describe("FormatBadge", () => {
  it("renders the format label", () => {
    render(<FormatBadge format="AAX" />);
    expect(screen.getByText("AAX")).toBeInTheDocument();
  });
});
