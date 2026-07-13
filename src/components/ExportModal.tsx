import { useFocusTrap } from "../useFocusTrap";

interface Props {
  count: number;
  selectedCount: number;
  onChoose: (which: "selected" | "shown") => void;
  onCancel: () => void;
}

export function ExportModal({ count, selectedCount, onChoose, onCancel }: Props) {
  const dialogRef = useFocusTrap<HTMLDivElement>();
  return (
    <div className="overlay" onClick={onCancel}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Export inventory"
        tabIndex={-1}
        ref={dialogRef}
        onClick={(e) => e.stopPropagation()}
      >
        <h2>Export inventory</h2>
        <p>You have {selectedCount} installs selected.</p>
        <div className="modal-actions">
          <button className="primary" onClick={() => onChoose("selected")}>
            Selected only ({selectedCount})
          </button>
          <button onClick={() => onChoose("shown")}>Everything shown ({count})</button>
          <button className="ghost" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
