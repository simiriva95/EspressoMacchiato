import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./i18n";
import "@fontsource/archivo-black";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./styles/global.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
