import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./App.css";
import { uiReady } from "./lib/api";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

requestAnimationFrame(() => requestAnimationFrame(() => { uiReady().catch(() => {}); }));
