import "./styles.css";
import { backend } from "./bridge";
import { createOverlay } from "./overlay";

const root = document.querySelector<HTMLElement>("#app");
if (!root) throw new Error("Missing application root");
const overlay = createOverlay(root, backend);
window.addEventListener("pagehide", () => overlay.dispose(), { once: true });
if (import.meta.hot) import.meta.hot.dispose(() => overlay.dispose());
