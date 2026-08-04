import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./i18n";
import "@fontsource/archivo-black";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./styles/global.css";

// macOS in-app: the window carries an NSVisualEffectView behind the webview
// (tauri windowEffects) — switch the tokens to their translucent variants.
if (
  "__TAURI_INTERNALS__" in window &&
  navigator.userAgent.toUpperCase().includes("MAC")
) {
  document.documentElement.dataset.glass = "true";
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
