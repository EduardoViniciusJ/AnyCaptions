import { afterEach, describe, expect, it, vi } from "vitest";
import { createOverlay } from "./overlay";
import type { AppSnapshot, Backend } from "./contracts";

function snapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return { revision: 0, sessionId: 0, mode: "en-en", capture: "stopped", windowMode: "edit",
    appearance: { fontSize: 32, color: "white" }, resources: { audioCapture: false, transcription: false, translation: false },
    canStart: false, canLock: false, error: null, ...overrides };
}

const disposers: (() => void)[] = [];
afterEach(() => { disposers.splice(0).forEach((dispose) => dispose()); document.body.replaceChildren(); });

function setup(initial = snapshot()) {
  const root = document.createElement("main");
  document.body.append(root);
  let listener: ((state: AppSnapshot) => void) | undefined;
  const release = vi.fn();
  const backend: Backend = {
    getState: vi.fn().mockResolvedValue(initial),
    subscribe: vi.fn(async (callback) => { listener = callback; return release; }),
    setMode: vi.fn(async (mode) => snapshot({ mode, revision: 1, sessionId: 1 })),
    setAppearance: vi.fn(async (appearance) => snapshot({ appearance, revision: 1 })),
    start: vi.fn().mockResolvedValue(snapshot({ capture: "preparing", revision: 1 })),
    stop: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    setWindowMode: vi.fn().mockRejectedValue({ kind: "domain", code: "recovery-unavailable" }),
    resize: vi.fn().mockResolvedValue(undefined), close: vi.fn().mockResolvedValue(undefined),
  };
  const overlay = createOverlay(root, backend);
  disposers.push(overlay.dispose);
  function get<T extends HTMLElement>(selector: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`Missing test element ${selector}`);
    return element;
  }
  return { root, backend, overlay, release, get, emit: (state: AppSnapshot) => listener?.(state) };
}

describe("overlay controls", () => {
  it("starts in edit mode with stopped capture and prevents unsafe actions", async () => {
    const app = setup();
    await app.overlay.ready;
    expect(app.get(".overlay").dataset.mode).toBe("edit");
    expect(app.get("#status").textContent).toBe("Captura parada");
    expect(app.get("#settings").hidden).toBe(true);
    expect(app.get("#status").closest("#settings")).not.toBeNull();
    expect(app.get<HTMLButtonElement>("#capture").disabled).toBe(true);
    expect(app.get<HTMLButtonElement>("#lock").disabled).toBe(true);
    app.get<HTMLButtonElement>("#capture").click();
    app.get<HTMLButtonElement>("#lock").click();
    expect(app.backend.start).not.toHaveBeenCalled();
    expect(app.backend.setWindowMode).not.toHaveBeenCalled();
  });

  it("changes modes through Rust and ignores stale event revisions", async () => {
    const app = setup();
    await app.overlay.ready;
    const select = app.get<HTMLSelectElement>("#language");
    select.value = "en-pt";
    select.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(select.disabled).toBe(false));
    expect(app.backend.setMode).toHaveBeenCalledWith("en-pt");
    app.emit(snapshot({ revision: 0 }));
    expect(select.value).toBe("en-pt");
    expect(app.get("#models-status").textContent).toContain("transcrição e tradução");
  });

  it("waits for backend confirmation and prevents concurrent control requests", async () => {
    const app = setup(snapshot({ canStart: true }));
    await app.overlay.ready;
    let resolve: ((value: AppSnapshot) => void) | undefined;
    vi.mocked(app.backend.start).mockImplementation(() => new Promise((done) => { resolve = done; }));
    const start = app.get<HTMLButtonElement>("#capture");
    start.click(); start.click();
    expect(app.backend.start).toHaveBeenCalledTimes(1);
    expect(app.get("#status").textContent).toBe("Captura parada");
    resolve?.(snapshot({ capture: "preparing", revision: 1 }));
    await vi.waitFor(() => expect(app.get("#status").textContent).toBe("Carregando modelos"));
    expect(app.get("#capture-label").textContent).toBe("Parar");
  });

  it("does not make controls part of the drag region", async () => {
    const app = setup();
    await app.overlay.ready;
    expect(app.root.querySelector("[data-tauri-drag-region]")).not.toBeNull();
    for (const control of app.root.querySelectorAll("button, select, input")) {
      expect(control.closest("[data-tauri-drag-region]")).toBeNull();
    }
  });

  it("opens settings and restores keyboard focus when Escape closes them", async () => {
    const app = setup();
    await app.overlay.ready;
    const toggle = app.get<HTMLButtonElement>("#settings-toggle");
    toggle.click();
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(app.get("#settings").hidden).toBe(false);
    app.get("#settings").dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(app.get("#settings").hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
  });

  it("applies only the appearance confirmed by the backend", async () => {
    const app = setup();
    await app.overlay.ready;
    const color = app.get<HTMLSelectElement>("#font-color");
    color.value = "yellow";
    color.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(app.get("#caption-preview").dataset.color).toBe("yellow"));
    expect(app.backend.setAppearance).toHaveBeenCalledWith({ fontSize: 32, color: "yellow" });
  });

  it("displays sanitized errors and restores confirmed values after rejection", async () => {
    const app = setup();
    await app.overlay.ready;
    vi.mocked(app.backend.setMode).mockRejectedValue("<img src=x onerror=alert(1)>");
    const select = app.get<HTMLSelectElement>("#language");
    select.value = "en-pt";
    select.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(app.get("#error").hidden).toBe(false));
    expect(app.root.querySelector("img")).toBeNull();
    expect(app.get("#error").textContent).toBe("Não foi possível concluir a ação. Tente novamente.");
    expect(select.value).toBe("en-en");
  });

  it("releases listeners and ignores events after disposal", async () => {
    const app = setup();
    await app.overlay.ready;
    app.overlay.dispose();
    app.emit(snapshot({ revision: 2, mode: "en-pt" }));
    expect(app.get<HTMLSelectElement>("#language").value).toBe("en-en");
    expect(app.release).toHaveBeenCalledOnce();
    app.get<HTMLButtonElement>("#close").click();
    expect(app.backend.close).not.toHaveBeenCalled();
  });

  it("starts native resizing only for the primary mouse button", async () => {
    const app = setup();
    await app.overlay.ready;
    app.get("#resize").dispatchEvent(new MouseEvent("mousedown", { button: 2 }));
    expect(app.backend.resize).not.toHaveBeenCalled();
    app.get("#resize").dispatchEvent(new MouseEvent("mousedown", { button: 0 }));
    expect(app.backend.resize).toHaveBeenCalledOnce();
  });
});
