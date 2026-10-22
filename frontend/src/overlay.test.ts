import { afterEach, describe, expect, it, vi } from "vitest";
import { createOverlay } from "./overlay";
import type { AppSnapshot, Backend, CaptionUpdate } from "./contracts";

function snapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return { revision: 0, sessionId: 0, mode: "en-en", capture: "stopped", windowMode: "edit",
    appearance: { fontSize: 32, color: "#FFFFFF", partialColor: "#B8B8B8" }, resources: { audioCapture: false, transcription: false, translation: false },
    canStart: false, canLock: false, shortcut: "Ctrl+Alt+L", shortcutAvailable: false, error: null, ...overrides };
}

const disposers: (() => void)[] = [];
afterEach(() => { disposers.splice(0).forEach((dispose) => dispose()); document.body.replaceChildren(); });

function setup(initial = snapshot()) {
  const root = document.createElement("main");
  document.body.append(root);
  let listener: ((state: AppSnapshot) => void) | undefined;
  let captionListener: ((caption: CaptionUpdate) => void) | undefined;
  const release = vi.fn();
  const backend: Backend = {
    getState: vi.fn().mockResolvedValue(initial),
    subscribe: vi.fn(async (callback) => { listener = callback; return release; }),
    subscribeCaptions: vi.fn(async (callback) => { captionListener = callback; return vi.fn(); }),
    setMode: vi.fn(async (mode) => snapshot({ mode, revision: 1, sessionId: 1 })),
    setAppearance: vi.fn(async (appearance) => snapshot({ appearance, revision: 1 })),
    start: vi.fn().mockResolvedValue(snapshot({ capture: "preparing", revision: 1 })),
    stop: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    setWindowMode: vi.fn().mockRejectedValue({ kind: "domain", code: "recovery-unavailable" }),
    setShortcut: vi.fn().mockResolvedValue(snapshot({ revision: 1, shortcutAvailable: true })),
    getAudioOutputs: vi.fn().mockResolvedValue({ devices: [], selectedId: null, selectedAvailable: false }),
    selectAudioOutput: vi.fn().mockResolvedValue({ devices: [], selectedId: null, selectedAvailable: false }),
    resize: vi.fn().mockResolvedValue(undefined), close: vi.fn().mockResolvedValue(undefined),
    fitSettings: vi.fn().mockResolvedValue(undefined), fitCaption: vi.fn().mockResolvedValue(undefined),
  };
  const overlay = createOverlay(root, backend);
  disposers.push(overlay.dispose);
  function get<T extends HTMLElement>(selector: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`Missing test element ${selector}`);
    return element;
  }
  return { root, backend, overlay, release, get, emit: (state: AppSnapshot) => listener?.(state),
    emitCaption: (caption: CaptionUpdate) => captionListener?.(caption) };
}

describe("overlay controls", () => {
  it("starts in edit mode with stopped capture and prevents unsafe actions", async () => {
    const app = setup();
    await app.overlay.ready;
    expect(app.get(".overlay").dataset.mode).toBe("edit");
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
    expect(app.get("#settings").hidden).toBe(true);
    expect(app.root.querySelector("#status, #availability, #models-status, #simulate")).toBeNull();
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
  });

  it("waits for backend confirmation and prevents concurrent control requests", async () => {
    const app = setup(snapshot({ canStart: true }));
    await app.overlay.ready;
    let resolve: ((value: AppSnapshot) => void) | undefined;
    vi.mocked(app.backend.start).mockImplementation(() => new Promise((done) => { resolve = done; }));
    const start = app.get<HTMLButtonElement>("#capture");
    start.click(); start.click();
    expect(app.backend.start).toHaveBeenCalledTimes(1);
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
    resolve?.(snapshot({ capture: "preparing", revision: 1 }));
    await vi.waitFor(() => expect(app.get("#capture-label").textContent).toBe("Parar"));
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
    expect(app.backend.fitSettings).toHaveBeenCalledOnce();
    app.get("#settings").dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(app.get("#settings").hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
    expect(app.backend.fitCaption).toHaveBeenCalledOnce();
  });

  it("previews appearance changes and applies them only after saving", async () => {
    const app = setup();
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#settings-toggle").click();
    const size = app.get<HTMLInputElement>("#font-size");
    const color = app.get<HTMLInputElement>("#font-color");
    const partialColor = app.get<HTMLInputElement>("#partial-color");
    size.value = "72";
    size.dispatchEvent(new Event("input"));
    color.value = "#33ccff";
    color.dispatchEvent(new Event("input"));
    partialColor.value = "#ff77aa";
    partialColor.dispatchEvent(new Event("input"));

    const preview = app.get<HTMLElement>("#appearance-preview");
    expect(preview.style.getPropertyValue("--caption-base-size")).toBe("72px");
    expect(preview.style.getPropertyValue("--caption-color")).toBe("#33CCFF");
    expect(preview.style.getPropertyValue("--caption-partial-color")).toBe("#FF77AA");
    expect(app.backend.setAppearance).not.toHaveBeenCalled();

    await vi.waitFor(() => expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false));
    app.get<HTMLButtonElement>("#save-settings").click();
    await vi.waitFor(() => expect(app.backend.setAppearance).toHaveBeenCalledWith({
      fontSize: 72,
      color: "#33CCFF",
      partialColor: "#FF77AA",
    }));
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
    expect(app.get<HTMLElement>("#caption-preview").style.getPropertyValue("--caption-color")).toBe("#33CCFF");
  });

  it("validates hexadecimal colors and restores the visual defaults", async () => {
    const app = setup(snapshot({ appearance: { fontSize: 48, color: "#00FF00", partialColor: "#0088FF" } }));
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#settings-toggle").click();
    const value = app.get<HTMLInputElement>("#font-color-value");
    value.value = "blue";
    value.dispatchEvent(new Event("input"));
    expect(value.getAttribute("aria-invalid")).toBe("true");
    expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(true);

    await vi.waitFor(() => expect(app.get<HTMLButtonElement>("#reset-appearance").disabled).toBe(false));
    app.get<HTMLButtonElement>("#reset-appearance").click();
    expect(app.get<HTMLInputElement>("#font-size").value).toBe("32");
    expect(value.value).toBe("#FFFFFF");
    expect(app.get<HTMLInputElement>("#partial-color-value").value).toBe("#B8B8B8");
    expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false);
  });

  it("sends the edited shortcut and reflects a locked state from Rust", async () => {
    const app = setup(snapshot({ canLock: true, shortcutAvailable: true }));
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#settings-toggle").click();
    const shortcut = app.get<HTMLInputElement>("#shortcut");
    shortcut.value = "Ctrl+Alt+K";
    await vi.waitFor(() => expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false));
    app.get<HTMLButtonElement>("#save-settings").click();
    await vi.waitFor(() => expect(app.backend.setShortcut).toHaveBeenCalledWith("Ctrl+Alt+K"));
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
    expect(app.get("#preview").hidden).toBe(false);
    app.emit(snapshot({ revision: 2, windowMode: "locked", canLock: true, shortcutAvailable: true }));
    expect(app.get(".overlay").dataset.mode).toBe("locked");
    expect(app.get("#settings").hidden).toBe(true);
  });

  it("keeps the caption visible when leaving edit mode", async () => {
    const app = setup(snapshot({ capture: "listening", canLock: true }));
    await app.overlay.ready;
    app.emitCaption({ sessionId: 0, segmentId: 1, revision: 1, mode: "en-en", text: "Still visible", isFinal: true });
    app.emit(snapshot({ revision: 1, capture: "listening", windowMode: "locked", canLock: true }));
    expect(app.get(".overlay").dataset.mode).toBe("locked");
    expect(app.get<HTMLElement>("#caption-live").hidden).toBe(false);
    expect(app.get("#caption-live").textContent).toBe("Still visible");
  });

  it("keeps settings open after a rejected save and closes them after a successful retry", async () => {
    const app = setup();
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#settings-toggle").click();
    await vi.waitFor(() => expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false));
    vi.mocked(app.backend.setShortcut).mockRejectedValueOnce({ code: "shortcut-unavailable" });
    app.get<HTMLInputElement>("#shortcut").value = "Ctrl+Alt+K";
    app.get<HTMLButtonElement>("#save-settings").click();
    await vi.waitFor(() => expect(app.get("#error").hidden).toBe(false));
    expect(app.get("#settings").hidden).toBe(false);
    await vi.waitFor(() => expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false));
    app.get<HTMLInputElement>("#shortcut").value = "Ctrl+Alt+K";
    app.get<HTMLButtonElement>("#save-settings").click();
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
  });

  it("keeps the listening view empty when there is no current caption", async () => {
    const app = setup(snapshot({ capture: "listening" }));
    await app.overlay.ready;
    expect(app.get("#caption-preview").hidden).toBe(true);
    expect(app.get("#caption-live").hidden).toBe(true);
    expect(app.root.textContent).not.toContain("Aguardando fala");
    expect(app.root.textContent).not.toContain("Áudio e modelos indisponíveis");
  });

  it("lists output names but selects by device ID", async () => {
    const app = setup();
    vi.mocked(app.backend.getAudioOutputs).mockResolvedValue({
      devices: [{ id: "speaker-a", name: "Alto-falantes" }, { id: "speaker-b", name: "Alto-falantes" }],
      selectedId: "speaker-b", selectedAvailable: true,
    });
    vi.mocked(app.backend.selectAudioOutput).mockResolvedValue({
      devices: [{ id: "speaker-a", name: "Alto-falantes" }, { id: "speaker-b", name: "Alto-falantes" }],
      selectedId: "speaker-a", selectedAvailable: true,
    });
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#settings-toggle").click();
    const select = app.get<HTMLSelectElement>("#audio-output");
    await vi.waitFor(() => expect(select.value).toBe("speaker-b"));
    expect([...select.options].map((option) => option.textContent)).toEqual(["Alto-falantes", "Alto-falantes"]);
    select.value = "speaker-a";
    select.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(app.backend.selectAudioOutput).toHaveBeenCalledWith("speaker-a"));
    await vi.waitFor(() => expect(select.value).toBe("speaker-a"));
  });

  it("shows a disconnected saved output without switching to another device", async () => {
    const app = setup();
    vi.mocked(app.backend.getAudioOutputs).mockResolvedValue({
      devices: [{ id: "speaker-b", name: "Monitor" }],
      selectedId: "speaker-a", selectedAvailable: false,
    });
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#settings-toggle").click();
    const select = app.get<HTMLSelectElement>("#audio-output");
    await vi.waitFor(() => expect(select.value).toBe("speaker-a"));
    expect(app.get("#audio-output-status").textContent).toContain("desconectada");
    expect(app.backend.selectAudioOutput).not.toHaveBeenCalled();
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

  it("starts horizontal resizing from either side only with the primary mouse button", async () => {
    const app = setup();
    await app.overlay.ready;
    app.get("#resize-right").dispatchEvent(new MouseEvent("mousedown", { button: 2 }));
    expect(app.backend.resize).not.toHaveBeenCalled();
    app.get("#resize-left").dispatchEvent(new MouseEvent("mousedown", { button: 0 }));
    app.get("#resize-right").dispatchEvent(new MouseEvent("mousedown", { button: 0 }));
    expect(app.backend.resize).toHaveBeenNthCalledWith(1, "West");
    expect(app.backend.resize).toHaveBeenNthCalledWith(2, "East");
  });
});
