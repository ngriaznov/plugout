import type { UpdateState } from "../updater";

interface Props {
  state: UpdateState;
  onDownload: () => void;
  onRestart: () => void;
}

/// Non-blocking update affordance in the toolbar. Renders nothing while idle;
/// otherwise walks available → downloading → ready alongside the update flow.
export function UpdatePill({ state, onDownload, onRestart }: Props) {
  switch (state.phase) {
    case "idle":
      return null;
    case "available":
      return (
        <button className="update-pill" onClick={onDownload}>
          v{state.version} available
        </button>
      );
    case "downloading":
      return (
        <span className="update-pill update-busy" role="status">
          <span className="spinner" />
          {state.percent === null ? "Downloading…" : `Downloading… ${state.percent}%`}
        </span>
      );
    case "ready":
      return (
        <button className="update-pill update-ready" onClick={onRestart}>
          Restart to update
        </button>
      );
    case "error":
      return (
        <span className="update-pill update-error" title={state.message}>
          Update failed
        </span>
      );
  }
}
