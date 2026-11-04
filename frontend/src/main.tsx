import { StrictMode } from "react";
import "./styles.css";
import { createRoot } from "react-dom/client";
import { DictationWindow } from "./app/DictationWindow";
import { App } from "./app/App";
import { backend } from "./bridge";

const element = document.getElementById("app");
if (!element) throw new Error("Missing application root");
const root = createRoot(element);
root.render(
  <StrictMode>
    {new URLSearchParams(window.location.search).get("view") === "dictation" ? <DictationWindow /> : <App backend={backend} />}
  </StrictMode>,
);
const dispose = () => root.unmount();
window.addEventListener("pagehide", dispose, { once: true });
if (import.meta.hot) import.meta.hot.dispose(dispose);
