// @vitest-environment jsdom
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { useFocusTrap } from "./useFocusTrap";

function Dialog() {
  const ref = useFocusTrap<HTMLDivElement>();
  return (
    <div ref={ref} role="dialog">
      <button>first</button>
      <button>second</button>
    </div>
  );
}

afterEach(cleanup);

describe("useFocusTrap", () => {
  it("focuses the first focusable on mount and cycles Tab", async () => {
    render(<><button>outside</button><Dialog /></>);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "first" }));
    await userEvent.tab(); // → second
    await userEvent.tab(); // wraps → first
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "first" }));
  });

  it("restores focus on unmount", async () => {
    const outside = () => screen.getByRole("button", { name: "outside" });
    const { rerender } = render(<><button>outside</button></>);
    outside().focus();
    rerender(<><button>outside</button><Dialog /></>);
    rerender(<><button>outside</button></>);
    expect(document.activeElement).toBe(outside());
  });
});
