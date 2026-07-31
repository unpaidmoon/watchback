import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { App } from "./App";
import { UnsavedChangesProvider } from "./shared/UnsavedChangesGuard";
import { installFrontendDiagnostics } from "./shared/frontendDiagnostics";
import "./styles.css";

installFrontendDiagnostics();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <HashRouter>
      <UnsavedChangesProvider>
        <App />
      </UnsavedChangesProvider>
    </HashRouter>
  </React.StrictMode>
);
