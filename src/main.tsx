import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

// The window starts hidden (`visible: false` in tauri.conf.json) so a cold
// launch never shows an unpainted shell. Reveal it after the first frame has
// been committed; the backend force-shows after 8s as a failsafe.
if ("__TAURI_INTERNALS__" in window) {
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      void import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
        getCurrentWindow().show().catch(() => {}),
      );
    }),
  );
}
