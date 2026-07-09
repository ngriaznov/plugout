import type { CSSProperties } from "react";
import type { Format } from "../types";

// Flat "Surveys"-style color blocks. Each component receives its color as the
// --c custom property; the stylesheet derives the pastel tint per theme.
export const FORMAT_COLORS: Record<Format, string> = {
  AU: "#a99bfa",
  VST3: "#a8c548",
  VST2: "#7db8f7",
  CLAP: "#f2d21f",
  AAX: "#f39a6b",
};

const colorVar = (format: Format) => ({ "--c": FORMAT_COLORS[format] }) as CSSProperties;

export function FormatBadge({ format }: { format: Format }) {
  return (
    <span className="badge" style={colorVar(format)}>
      {format}
    </span>
  );
}

// Interactive variant: toggles selection of one format install of a plugin.
export function FormatChip({
  format,
  selected,
  onToggle,
}: {
  format: Format;
  selected: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={`chip${selected ? " chip-on" : ""}`}
      aria-pressed={selected}
      title={selected ? `Deselect ${format}` : `Select ${format}`}
      style={colorVar(format)}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
    >
      {format}
    </button>
  );
}
