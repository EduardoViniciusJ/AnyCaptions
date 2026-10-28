import { afterEach, describe, expect, it, vi } from "vitest";
import { createOverlay } from "./overlay";
import type { AppSnapshot, Backend, CaptionUpdate, ModelDownloadUpdate } from "./contracts";

const RIVA_MODEL = "riva-translate-4b-instruct-v2-q4_k_m";
const RIVA_BYTES = 2_759_302_368;

function snapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return { revision: 0, sessionId: 0, uiLanguage: "pt-br", mode: "auto", translationEngine: "opus", sourceLanguage: null, capture: "stopped", windowMode: "edit",
    appearance: { fontSize: 32, color: "#FFFFFF", partialColor: "#B8B8B8", background: false }, resources: { audioCapture: false, transcription: false, translation: false },
    canStart: false, canLock: false, shortcut: "Ctrl+Alt+L", shortcutAvailable: false, error: null, ...overrides };
}

const disposers: (() => void)[] = [];
afterEach(() => { disposers.splice(0).forEach((dispose) => dispose()); document.body.replaceChildren(); });

function setup(initial = snapshot()) {
  const root = document.createElement("main");
  document.body.append(root);
  let listener: ((state: AppSnapshot) => void) | undefined;
  let captionListener: ((caption: CaptionUpdate) => void) | undefined;
  let modelDownloadListener: ((update: ModelDownloadUpdate) => void) | undefined;
  const release = vi.fn();
  const backend: Backend = {
    getState: vi.fn().mockResolvedValue(initial),
    subscribe: vi.fn(async (callback) => { listener = callback; return release; }),
    subscribeCaptions: vi.fn(async (callback) => { captionListener = callback; return vi.fn(); }),
    setMode: vi.fn(async (mode) => snapshot({ mode, revision: 1, sessionId: 1 })),
    setTranslationEngine: vi.fn(async (translationEngine) => snapshot({ translationEngine, revision: 1, sessionId: 1 })),
    setSourceLanguage: vi.fn(async (sourceLanguage) => snapshot({ sourceLanguage, revision: 1, sessionId: 1 })),
    setUiLanguage: vi.fn(async (uiLanguage) => snapshot({ uiLanguage, revision: 1 })),
    setAppearance: vi.fn(async (appearance) => snapshot({ appearance, revision: 1 })),
    start: vi.fn().mockResolvedValue(snapshot({ capture: "preparing", revision: 1 })),
    stop: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    setWindowMode: vi.fn().mockRejectedValue({ kind: "domain", code: "recovery-unavailable" }),
    setShortcut: vi.fn().mockResolvedValue(snapshot({ revision: 1, shortcutAvailable: true })),
    getAudioOutputs: vi.fn().mockResolvedValue({ devices: [], selectedId: null, selectedAvailable: false }),
    selectAudioOutput: vi.fn().mockResolvedValue({ devices: [], selectedId: null, selectedAvailable: false }),
    resize: vi.fn().mockResolvedValue(undefined), close: vi.fn().mockResolvedValue(undefined),
    openProjectPage: vi.fn().mockResolvedValue(undefined),
    getDictation: vi.fn().mockResolvedValue({
      settings: {
        enabled: false,
        shortcut: "Ctrl+Space",
        activation: "hold",
        soundCue: true,
        showLiveText: true,
        limitDuration: true,
        maxDurationSecs: 60,
        inputId: null,
        language: null,
        translateTo: null,
      },
      available: true,
      phase: "idle",
      inputs: { devices: [], selectedId: null, selectedAvailable: false },
    }),
    setDictation: vi.fn(async (settings) => ({
      settings,
      available: true,
      phase: "idle" as const,
      inputs: { devices: [], selectedId: null, selectedAvailable: false },
    })),
    subscribeDictation: vi.fn(async () => vi.fn()),
    getModelDownloads: vi.fn().mockResolvedValue([
      { model: RIVA_MODEL, engine: "riva", installed: false, downloadable: true, downloading: false, totalBytes: RIVA_BYTES },
    ]),
    downloadModel: vi.fn().mockResolvedValue(undefined),
    cancelModelDownload: vi.fn().mockResolvedValue(undefined),
    subscribeModelDownloads: vi.fn(async (callback) => { modelDownloadListener = callback; return vi.fn(); }),
  };
  const overlay = createOverlay(root, backend);
  disposers.push(overlay.dispose);
  function get<T extends HTMLElement>(selector: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`Missing test element ${selector}`);
    return element;
  }
  return { root, backend, overlay, release, get, emit: (state: AppSnapshot) => listener?.(state),
    emitCaption: (caption: CaptionUpdate) => captionListener?.(caption),
    emitModelDownload: (update: ModelDownloadUpdate) => modelDownloadListener?.(update) };
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

  it("lists every translation target with a flag and disables them until the model is available", async () => {
    const app = setup();
    await app.overlay.ready;
    const items = app.root.querySelectorAll("#language-menu li");
    expect(items.length).toBe(38);
    const japanese = app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]');
    expect(japanese.textContent).toContain("Traduzir para JA");
    expect(japanese.querySelector("img.flag")).not.toBeNull();
    const portuguese = app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]');
    expect(portuguese.getAttribute("aria-disabled")).toBe("true");
    app.emit(snapshot({ revision: 1, resources: { audioCapture: true, transcription: true, translation: true } }));
    expect(portuguese.getAttribute("aria-disabled")).toBe("false");
  });

  it("pins the caption source language through Rust and reflects the snapshot", async () => {
    const app = setup();
    await app.overlay.ready;
    const items = app.root.querySelectorAll("#source-language-menu li");
    expect(items.length).toBe(84); // automático + 83 idiomas do modelo
    expect(app.get("#source-language-label").textContent).toBe("Automático");
    const portuguese = app.get<HTMLLIElement>('#source-language-menu li[data-value="pt-BR"]');
    expect(portuguese.querySelector("img.flag")).not.toBeNull();
    portuguese.click();
    expect(app.backend.setSourceLanguage).toHaveBeenCalledWith("pt-BR");
    await app.overlay.ready;
    await Promise.resolve();
    expect(app.get("#source-language-label").textContent).toBe("Português (Brasil)");
    // Escolher o mesmo valor de novo não chama o backend outra vez.
    portuguese.click();
    expect(app.backend.setSourceLanguage).toHaveBeenCalledTimes(1);
  });

  it("offers dictation translation targets filtered by the chosen engine", async () => {
    const app = setup();
    await app.overlay.ready;
    const items = app.root.querySelectorAll<HTMLLIElement>("#dictation-translate-menu li");
    expect(items.length).toBe(38); // "Não traduzir" + 37 alvos do catálogo
    expect(app.get("#dictation-translate-label").textContent).toBe("Não traduzir");
    // Mecanismo OPUS (padrão): só EN/PT visíveis, como no menu das legendas.
    const japanese = app.get<HTMLLIElement>('#dictation-translate-menu li[data-value="ja"]');
    const english = app.get<HTMLLIElement>('#dictation-translate-menu li[data-value="en"]');
    expect(japanese.hidden).toBe(true);
    expect(english.hidden).toBe(false);
    expect(english.querySelector("img.flag")).not.toBeNull();
    english.click();
    expect(app.get("#dictation-translate-label").textContent).toBe("Inglês");
    expect(english.getAttribute("aria-selected")).toBe("true");
  });

  it("lists every dictation language of the model with a flag and applies the choice", async () => {
    const app = setup();
    await app.overlay.ready;
    const items = app.root.querySelectorAll("#dictation-language-menu li");
    expect(items.length).toBe(84); // automático + 83 idiomas do dicionário do modelo
    expect(app.get("#dictation-language-label").textContent).toBe("Automático");
    const portuguese = app.get<HTMLLIElement>('#dictation-language-menu li[data-value="pt-BR"]');
    expect(portuguese.querySelector("img.flag")).not.toBeNull();
    const japanese = app.get<HTMLLIElement>('#dictation-language-menu li[data-value="ja-JP"]');
    expect(japanese.querySelector("img.flag")).not.toBeNull();
    portuguese.click();
    expect(app.get("#dictation-language-label").textContent).toBe("Português (Brasil)");
    expect(portuguese.getAttribute("aria-selected")).toBe("true");
  });

  it("offers only the chosen engine's targets and switches engines through Rust", async () => {
    const app = setup(snapshot({ resources: { audioCapture: true, transcription: true, translation: true } }));
    await app.overlay.ready;
    // OPUS (padrão): apenas os alvos EN/PT permanecem visíveis no menu.
    expect(app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]').hidden).toBe(true);
    expect(app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]').hidden).toBe(false);
    expect(app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-pt"]').hidden).toBe(false);
    expect(app.get<HTMLLIElement>('#language-menu li[data-value="translate-en"]').hidden).toBe(false);
    app.get<HTMLButtonElement>("#settings-toggle").click();
    const select = app.get<HTMLSelectElement>("#translation-engine");
    expect(select.value).toBe("opus");
    select.value = "riva";
    select.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(app.backend.setTranslationEngine).toHaveBeenCalledWith("riva"));
    app.emit(snapshot({ revision: 2, translationEngine: "riva", resources: { audioCapture: true, transcription: true, translation: true } }));
    expect(select.value).toBe("riva");
    expect(app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]').hidden).toBe(false);
  });

  it("switches the interface language and applies the texts", async () => {
    const app = setup();
    await app.overlay.ready;
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
    app.emit(snapshot({ revision: 1, uiLanguage: "en" }));
    expect(app.get("#capture-label").textContent).toBe("Start");
    expect(app.get("#settings h2").textContent).toBe("Settings");
    expect(app.get("#ui-language-label").textContent).toBe("English");
    expect(app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]').textContent)
      .toContain("Translate to JA");
    // Escolher outro idioma no menu chama o backend.
    app.get<HTMLButtonElement>("#ui-language").click();
    app.get<HTMLLIElement>('#ui-language-menu li[data-value="ru"]').click();
    await vi.waitFor(() => expect(app.backend.setUiLanguage).toHaveBeenCalledWith("ru"));
    // Volta para o português para os demais testes.
    app.emit(snapshot({ revision: 2, uiLanguage: "pt-br" }));
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
  });

  it("changes modes through Rust and ignores stale event revisions", async () => {
    const app = setup(snapshot({ resources: { audioCapture: true, transcription: true, translation: true } }));
    await app.overlay.ready;
    const button = app.get<HTMLButtonElement>("#language");
    const menu = app.get<HTMLUListElement>("#language-menu");
    button.click();
    expect(menu.hidden).toBe(false);
    app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]').click();
    expect(menu.hidden).toBe(true);
    await vi.waitFor(() => expect(button.disabled).toBe(false));
    expect(app.backend.setMode).toHaveBeenCalledWith("translate-pt-br");
    app.emit(snapshot({ revision: 0 }));
    expect(app.get("#language-label").textContent).toBe("Traduzir para PT");
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
    app.get("#settings").dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(app.get("#settings").hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
  });

  it("opens the project page when the developer credit is clicked", async () => {
    const app = setup();
    await app.overlay.ready;
    const credit = app.get<HTMLButtonElement>("#developer-credit");
    expect(credit.textContent).toBe("developed by EduardoViniciusJ");
    credit.click();
    expect(app.backend.openProjectPage).toHaveBeenCalledTimes(1);
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
    const background = app.get<HTMLInputElement>("#caption-background");
    background.checked = true;
    background.dispatchEvent(new Event("change"));

    const preview = app.get<HTMLElement>("#appearance-preview");
    expect(preview.style.getPropertyValue("--caption-base-size")).toBe("72px");
    expect(preview.style.getPropertyValue("--caption-color")).toBe("#33CCFF");
    expect(preview.style.getPropertyValue("--caption-partial-color")).toBe("#FF77AA");
    expect(preview.hasAttribute("data-background")).toBe(true);
    expect(app.get("#caption-preview").hasAttribute("data-background")).toBe(false);
    expect(app.backend.setAppearance).not.toHaveBeenCalled();

    await vi.waitFor(() => expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false));
    app.get<HTMLButtonElement>("#save-settings").click();
    await vi.waitFor(() => expect(app.backend.setAppearance).toHaveBeenCalledWith({
      fontSize: 72,
      color: "#33CCFF",
      partialColor: "#FF77AA",
      background: true,
    }));
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
    expect(app.get<HTMLElement>("#caption-preview").style.getPropertyValue("--caption-color")).toBe("#33CCFF");
    expect(app.get("#caption-preview").hasAttribute("data-background")).toBe(true);
  });

  it("validates hexadecimal colors and restores the visual defaults", async () => {
    const app = setup(snapshot({ appearance: { fontSize: 48, color: "#00FF00", partialColor: "#0088FF", background: true } }));
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
    expect(app.get<HTMLInputElement>("#caption-background").checked).toBe(false);
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
    app.emitCaption({ sessionId: 0, segmentId: 1, revision: 1, mode: "auto", text: "Still visible", isFinal: true, language: "en-US" });
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
    const app = setup(snapshot({ resources: { audioCapture: true, transcription: true, translation: true } }));
    await app.overlay.ready;
    vi.mocked(app.backend.setMode).mockRejectedValue("<img src=x onerror=alert(1)>");
    app.get<HTMLButtonElement>("#language").click();
    app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]').click();
    await vi.waitFor(() => expect(app.get("#error").hidden).toBe(false));
    expect(app.get("#error").querySelector("img")).toBeNull();
    expect(app.get("#error").textContent).toBe("Não foi possível concluir a ação. Tente novamente.");
    expect(app.get("#language-label").textContent).toBe("Automático");
  });

  it("releases listeners and ignores events after disposal", async () => {
    const app = setup();
    await app.overlay.ready;
    app.overlay.dispose();
    app.emit(snapshot({ revision: 2, mode: "translate-pt-br" }));
    expect(app.get("#language-label").textContent).toBe("Automático");
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

describe("model download", () => {
  it("offers the download only while the selected engine's model is missing", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.overlay.ready;
    const row = app.get("#model-download");
    expect(row.hidden).toBe(false);
    expect(app.get("#model-download-status").textContent).toBe(
      "O modelo deste mecanismo ainda não foi baixado.",
    );
    expect(app.get("#model-download-action").textContent).toBe("Baixar (2.8 GB)");
    app.emit(snapshot({ revision: 1, translationEngine: "opus" }));
    expect(row.hidden).toBe(true);
  });

  it("starts the download, shows progress and lets the user cancel", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.overlay.ready;
    app.get<HTMLButtonElement>("#model-download-action").click();
    expect(app.backend.downloadModel).toHaveBeenCalledWith(RIVA_MODEL);
    app.emitModelDownload({
      model: RIVA_MODEL,
      phase: "downloading",
      receivedBytes: RIVA_BYTES / 2,
      totalBytes: RIVA_BYTES,
      reason: null,
    });
    expect(app.get("#model-download-status").textContent).toBe("Baixando… 50%");
    const progress = app.get<HTMLProgressElement>("#model-download-progress");
    expect(progress.hidden).toBe(false);
    expect(progress.value).toBe(50);
    app.get<HTMLButtonElement>("#model-download-action").click();
    expect(app.backend.cancelModelDownload).toHaveBeenCalled();
  });

  it("explains a failure and offers a retry", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.overlay.ready;
    app.emitModelDownload({
      model: RIVA_MODEL,
      phase: "failed",
      receivedBytes: 0,
      totalBytes: null,
      reason: "corrupted",
    });
    expect(app.get("#model-download-status").textContent).toBe(
      "O arquivo baixado não passou na verificação. Tente de novo.",
    );
    expect(app.get("#model-download-action").textContent).toBe("Tentar de novo");
    expect(app.get<HTMLProgressElement>("#model-download-progress").hidden).toBe(true);
  });

  it("hides the offer once the installed model arrives", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.overlay.ready;
    vi.mocked(app.backend.getModelDownloads).mockResolvedValue([
      { model: RIVA_MODEL, engine: "riva", installed: true, downloadable: true, downloading: false, totalBytes: RIVA_BYTES },
    ]);
    app.emitModelDownload({
      model: RIVA_MODEL,
      phase: "ready",
      receivedBytes: RIVA_BYTES,
      totalBytes: RIVA_BYTES,
      reason: null,
    });
    await vi.waitFor(() => expect(app.get("#model-download").hidden).toBe(true));
  });
});
