import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render } from "@testing-library/react";
import { createElement, StrictMode } from "react";
import { App } from "./app/App";
import type {
  AppSnapshot,
  Backend,
  CaptionUpdate,
  ModelDownloadUpdate,
} from "./contracts";

const RIVA_MODEL = "riva-translate-4b-instruct-v2-q4_k_m";
const RIVA_BYTES = 2_759_302_368;

function snapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return {
    revision: 0,
    sessionId: 0,
    uiLanguage: "pt-br",
    mode: "auto",
    translationEngine: "opus",
    sourceLanguage: null,
    speakerLabels: false,
    capture: "stopped",
    windowMode: "edit",
    appearance: {
      fontSize: 32,
      color: "#FFFFFF",
      partialColor: "#B8B8B8",
      background: false,
      font: "inter",
    },
    resources: {
      audioCapture: false,
      transcription: false,
      translation: false,
      speakerLabels: false,
    },
    canStart: false,
    canLock: false,
    shortcut: "Ctrl+Alt+L",
    shortcutAvailable: false,
    error: null,
    version: "0.1.1",
    updateCheck: false,
    ...overrides,
  };
}

const disposers: (() => void)[] = [];
afterEach(() => {
  disposers.splice(0).forEach((dispose) => dispose());
  document.body.replaceChildren();
});

function setup(initial = snapshot()) {
  const root = document.createElement("main");
  document.body.append(root);
  let listener: ((state: AppSnapshot) => void) | undefined;
  let captionListener: ((caption: CaptionUpdate) => void) | undefined;
  let modelDownloadListener:
    ((update: ModelDownloadUpdate) => void) | undefined;
  const release = vi.fn();
  const backend: Backend = {
    getState: vi.fn().mockResolvedValue(initial),
    subscribe: vi.fn(async (callback) => {
      listener = callback;
      return release;
    }),
    subscribeCaptions: vi.fn(async (callback) => {
      captionListener = callback;
      return vi.fn();
    }),
    setMode: vi.fn(async (mode) =>
      snapshot({ mode, revision: 1, sessionId: 1 }),
    ),
    setTranslationEngine: vi.fn(async (translationEngine) =>
      snapshot({ translationEngine, revision: 1, sessionId: 1 }),
    ),
    setSpeakerLabels: vi.fn(async (speakerLabels) =>
      snapshot({ speakerLabels, revision: 1, sessionId: 1 }),
    ),
    setUpdateCheck: vi.fn(async (updateCheck) =>
      snapshot({ updateCheck, revision: 1 }),
    ),
    checkForUpdates: vi.fn().mockResolvedValue({ current: "0.1.1", latest: "0.1.1", available: false }),
    openReleasesPage: vi.fn().mockResolvedValue(undefined),
    setSourceLanguage: vi.fn(async (sourceLanguage) =>
      snapshot({ sourceLanguage, revision: 1, sessionId: 1 }),
    ),
    setUiLanguage: vi.fn(async (uiLanguage) =>
      snapshot({ uiLanguage, revision: 1 }),
    ),
    setAppearance: vi.fn(async (appearance) =>
      snapshot({ appearance, revision: 1 }),
    ),
    start: vi
      .fn()
      .mockResolvedValue(snapshot({ capture: "preparing", revision: 1 })),
    stop: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    setWindowMode: vi
      .fn()
      .mockRejectedValue({ kind: "domain", code: "recovery-unavailable" }),
    setShortcut: vi
      .fn()
      .mockResolvedValue(snapshot({ revision: 1, shortcutAvailable: true })),
    getAudioOutputs: vi.fn().mockResolvedValue({
      devices: [],
      selectedId: null,
      selectedAvailable: false,
    }),
    selectAudioOutput: vi.fn().mockResolvedValue({
      devices: [],
      selectedId: null,
      selectedAvailable: false,
    }),
    resize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
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
      {
        model: RIVA_MODEL,
        kind: "translation",
        engine: "riva",
        installed: false,
        downloadable: true,
        downloading: false,
        totalBytes: RIVA_BYTES,
      },
    ]),
    downloadModel: vi.fn().mockResolvedValue(undefined),
    cancelModelDownload: vi.fn().mockResolvedValue(undefined),
    subscribeModelDownloads: vi.fn(async (callback) => {
      modelDownloadListener = callback;
      return vi.fn();
    }),
  };
  const view = render(createElement(App, { backend }), { container: root });
  const overlay = { dispose: () => view.unmount() };
  disposers.push(overlay.dispose);
  const ready = async () => {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  };
  function get<T extends HTMLElement>(selector: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`Missing test element ${selector}`);
    return element;
  }
  return {
    root,
    backend,
    overlay,
    ready,
    release,
    get,
    emit: (state: AppSnapshot) => act(() => listener?.(state)),
    emitCaption: (caption: CaptionUpdate) =>
      act(() => captionListener?.(caption)),
    emitModelDownload: (update: ModelDownloadUpdate) =>
      act(() => modelDownloadListener?.(update)),
  };
}

describe("overlay controls", () => {
  it("starts in edit mode with stopped capture and prevents unsafe actions", async () => {
    const app = setup();
    await app.ready();
    expect(app.get(".overlay").dataset.mode).toBe("edit");
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
    expect(app.get("#settings").hidden).toBe(true);
    expect(
      app.root.querySelector(
        "#status, #availability, #models-status, #simulate",
      ),
    ).toBeNull();
    expect(app.get<HTMLButtonElement>("#capture").disabled).toBe(true);
    expect(app.get<HTMLButtonElement>("#lock").disabled).toBe(true);
    fireEvent.click(app.get<HTMLButtonElement>("#capture"));
    fireEvent.click(app.get<HTMLButtonElement>("#lock"));
    expect(app.backend.start).not.toHaveBeenCalled();
    expect(app.backend.setWindowMode).not.toHaveBeenCalled();
  });

  it("lists every translation target with a flag and disables them until the model is available", async () => {
    const app = setup();
    await app.ready();
    const items = app.root.querySelectorAll('#language-menu li[role="option"]');
    expect(items.length).toBe(38);
    const japanese = app.get<HTMLLIElement>(
      '#language-menu li[data-value="translate-ja"]',
    );
    expect(japanese.textContent).toContain("Traduzir para JA");
    expect(japanese.querySelector("img.flag")).not.toBeNull();
    const portuguese = app.get<HTMLLIElement>(
      '#language-menu li[data-value="translate-pt-br"]',
    );
    expect(portuguese.getAttribute("aria-disabled")).toBe("true");
    app.emit(
      snapshot({
        revision: 1,
        resources: {
          audioCapture: true,
          transcription: true,
          translation: true,
          speakerLabels: false,
        },
      }),
    );
    expect(portuguese.getAttribute("aria-disabled")).toBe("false");
  });

  it("pins the caption source language through Rust and reflects the snapshot", async () => {
    const app = setup();
    await app.ready();
    const items = app.root.querySelectorAll("#source-language-menu li");
    expect(items.length).toBe(84); // automático + 83 idiomas do modelo
    expect(app.get("#source-language-label").textContent).toBe("Automático");
    const portuguese = app.get<HTMLLIElement>(
      '#source-language-menu li[data-value="pt-BR"]',
    );
    expect(portuguese.querySelector("img.flag")).not.toBeNull();
    fireEvent.click(portuguese);
    expect(app.backend.setSourceLanguage).toHaveBeenCalledWith("pt-BR");
    await app.ready();
    await Promise.resolve();
    expect(app.get("#source-language-label").textContent).toBe(
      "Português (Brasil)",
    );
    // Escolher o mesmo valor de novo não chama o backend outra vez.
    fireEvent.click(portuguese);
    expect(app.backend.setSourceLanguage).toHaveBeenCalledTimes(1);
  });

  it("offers dictation translation targets filtered by the chosen engine", async () => {
    const app = setup();
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    await app.ready();
    const items = app.root.querySelectorAll<HTMLLIElement>(
      "#dictation-translate-menu li",
    );
    expect(items.length).toBe(38); // "Não traduzir" + 37 alvos do catálogo
    expect(app.get("#dictation-translate-label").textContent).toBe(
      "Não traduzir",
    );
    // Mecanismo OPUS (padrão): só EN/PT visíveis, como no menu das legendas.
    const japanese = app.get<HTMLLIElement>(
      '#dictation-translate-menu li[data-value="ja"]',
    );
    const english = app.get<HTMLLIElement>(
      '#dictation-translate-menu li[data-value="en"]',
    );
    expect(japanese.hidden).toBe(true);
    expect(english.hidden).toBe(false);
    expect(english.querySelector("img.flag")).not.toBeNull();
    fireEvent.click(english);
    expect(app.get("#dictation-translate-label").textContent).toBe("Inglês");
    expect(english.getAttribute("aria-selected")).toBe("true");
  });

  it("lists every dictation language of the model with a flag and applies the choice", async () => {
    const app = setup();
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    await app.ready();
    const items = app.root.querySelectorAll("#dictation-language-menu li");
    expect(items.length).toBe(84); // automático + 83 idiomas do dicionário do modelo
    expect(app.get("#dictation-language-label").textContent).toBe("Automático");
    const portuguese = app.get<HTMLLIElement>(
      '#dictation-language-menu li[data-value="pt-BR"]',
    );
    expect(portuguese.querySelector("img.flag")).not.toBeNull();
    const japanese = app.get<HTMLLIElement>(
      '#dictation-language-menu li[data-value="ja-JP"]',
    );
    expect(japanese.querySelector("img.flag")).not.toBeNull();
    fireEvent.click(portuguese);
    expect(app.get("#dictation-language-label").textContent).toBe(
      "Português (Brasil)",
    );
    expect(portuguese.getAttribute("aria-selected")).toBe("true");
  });

  it("offers only the chosen engine's targets and switches engines through Rust", async () => {
    const app = setup(
      snapshot({
        resources: {
          audioCapture: true,
          transcription: true,
          translation: true,
          speakerLabels: false,
        },
      }),
    );
    await app.ready();
    // OPUS (padrão): apenas os alvos EN/PT permanecem visíveis no menu.
    expect(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]')
        .hidden,
    ).toBe(true);
    expect(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]')
        .hidden,
    ).toBe(false);
    expect(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-pt"]')
        .hidden,
    ).toBe(false);
    expect(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-en"]')
        .hidden,
    ).toBe(false);
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    const select = app.get<HTMLSelectElement>("#translation-engine");
    expect(select.value).toBe("opus");
    fireEvent.change(select, { target: { value: "riva" } });
    await vi.waitFor(() =>
      expect(app.backend.setTranslationEngine).toHaveBeenCalledWith("riva"),
    );
    app.emit(
      snapshot({
        revision: 2,
        translationEngine: "riva",
        resources: {
          audioCapture: true,
          transcription: true,
          translation: true,
          speakerLabels: false,
        },
      }),
    );
    expect(select.value).toBe("riva");
    expect(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]')
        .hidden,
    ).toBe(false);
  });

  it("switches the interface language and applies the texts", async () => {
    const app = setup();
    await app.ready();
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
    app.emit(snapshot({ revision: 1, uiLanguage: "en" }));
    expect(app.get("#capture-label").textContent).toBe("Start");
    expect(app.get("#settings h2").textContent).toBe("Settings");
    expect(app.get("#ui-language-label").textContent).toBe("English");
    expect(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-ja"]')
        .textContent,
    ).toContain("Translate to JA");
    // Escolher outro idioma no menu chama o backend.
    fireEvent.click(app.get<HTMLButtonElement>("#ui-language"));
    fireEvent.click(
      app.get<HTMLLIElement>('#ui-language-menu li[data-value="ru"]'),
    );
    await vi.waitFor(() =>
      expect(app.backend.setUiLanguage).toHaveBeenCalledWith("ru"),
    );
    // Volta para o português para os demais testes.
    app.emit(snapshot({ revision: 2, uiLanguage: "pt-br" }));
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
  });

  it("changes modes through Rust and ignores stale event revisions", async () => {
    const app = setup(
      snapshot({
        resources: {
          audioCapture: true,
          transcription: true,
          translation: true,
          speakerLabels: false,
        },
      }),
    );
    await app.ready();
    const button = app.get<HTMLButtonElement>("#language");
    const menu = app.get<HTMLUListElement>("#language-menu");
    fireEvent.click(button);
    expect(menu.hidden).toBe(false);
    fireEvent.click(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]'),
    );
    expect(menu.hidden).toBe(true);
    await vi.waitFor(() => expect(button.disabled).toBe(false));
    expect(app.backend.setMode).toHaveBeenCalledWith("translate-pt-br");
    app.emit(snapshot({ revision: 0 }));
    expect(app.get("#language-label").textContent).toBe("Traduzir para PT");
  });

  it("waits for backend confirmation and prevents concurrent control requests", async () => {
    const app = setup(snapshot({ canStart: true }));
    await app.ready();
    let resolve: ((value: AppSnapshot) => void) | undefined;
    vi.mocked(app.backend.start).mockImplementation(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const start = app.get<HTMLButtonElement>("#capture");
    fireEvent.click(start);
    fireEvent.click(start);
    expect(app.backend.start).toHaveBeenCalledTimes(1);
    expect(app.get("#capture-label").textContent).toBe("Iniciar");
    resolve?.(snapshot({ capture: "preparing", revision: 1 }));
    await vi.waitFor(() =>
      expect(app.get("#capture-label").textContent).toBe("Parar"),
    );
  });

  it("does not make controls part of the drag region", async () => {
    const app = setup();
    await app.ready();
    expect(app.root.querySelector("[data-tauri-drag-region]")).not.toBeNull();
    for (const control of app.root.querySelectorAll("button, select, input")) {
      expect(control.closest("[data-tauri-drag-region]")).toBeNull();
    }
  });

  it("opens settings and restores keyboard focus when Escape closes them", async () => {
    const app = setup();
    await app.ready();
    const toggle = app.get<HTMLButtonElement>("#settings-toggle");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(app.get("#settings").hidden).toBe(false);
    fireEvent(
      app.get("#settings"),
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    expect(app.get("#settings").hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
  });

  it("opens the project page when the developer credit is clicked", async () => {
    const app = setup();
    await app.ready();
    const credit = app.get<HTMLButtonElement>("#developer-credit");
    expect(credit.textContent).toContain("EduardoViniciusJ");
    fireEvent.click(credit);
    expect(app.backend.openProjectPage).toHaveBeenCalledTimes(1);
  });

  it("previews appearance changes and applies them only after saving", async () => {
    const app = setup();
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    const size = app.get<HTMLInputElement>("#font-size");
    const color = app.get<HTMLInputElement>("#font-color");
    const partialColor = app.get<HTMLInputElement>("#partial-color");
    fireEvent.change(size, { target: { value: "72" } });
    fireEvent.change(color, { target: { value: "#33ccff" } });
    fireEvent.change(partialColor, { target: { value: "#ff77aa" } });
    const background = app.get<HTMLInputElement>("#caption-background");
    fireEvent.click(background);

    const preview = app.get<HTMLElement>("#appearance-preview");
    expect(preview.style.getPropertyValue("--caption-base-size")).toBe("72px");
    expect(preview.style.getPropertyValue("--caption-color")).toBe("#33CCFF");
    expect(preview.style.getPropertyValue("--caption-partial-color")).toBe(
      "#FF77AA",
    );
    expect(preview.hasAttribute("data-background")).toBe(true);
    expect(app.get("#caption-preview").hasAttribute("data-background")).toBe(
      false,
    );
    expect(app.backend.setAppearance).not.toHaveBeenCalled();

    await vi.waitFor(() =>
      expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false),
    );
    fireEvent.click(app.get<HTMLButtonElement>("#save-settings"));
    await vi.waitFor(() =>
      expect(app.backend.setAppearance).toHaveBeenCalledWith({
        fontSize: 72,
        color: "#33CCFF",
        partialColor: "#FF77AA",
        background: true,
        font: "inter",
      }),
    );
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
    expect(
      app
        .get<HTMLElement>("#caption-preview")
        .style.getPropertyValue("--caption-color"),
    ).toBe("#33CCFF");
    expect(app.get("#caption-preview").hasAttribute("data-background")).toBe(
      true,
    );
  });

  it("validates hexadecimal colors and restores the visual defaults", async () => {
    const app = setup(
      snapshot({
        appearance: {
          fontSize: 48,
          color: "#00FF00",
          partialColor: "#0088FF",
          background: true,
          font: "inter",
        },
      }),
    );
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    const value = app.get<HTMLInputElement>("#font-color-value");
    fireEvent.change(value, { target: { value: "blue" } });
    expect(value.getAttribute("aria-invalid")).toBe("true");
    expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(true);

    await vi.waitFor(() =>
      expect(app.get<HTMLButtonElement>("#reset-appearance").disabled).toBe(
        false,
      ),
    );
    fireEvent.click(app.get<HTMLButtonElement>("#reset-appearance"));
    expect(app.get<HTMLInputElement>("#font-size").value).toBe("32");
    expect(value.value).toBe("#FFFFFF");
    expect(app.get<HTMLInputElement>("#partial-color-value").value).toBe(
      "#B8B8B8",
    );
    expect(app.get<HTMLInputElement>("#caption-background").checked).toBe(
      false,
    );
    expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false);
  });

  it("sends the edited shortcut and reflects a locked state from Rust", async () => {
    const app = setup(snapshot({ canLock: true, shortcutAvailable: true }));
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    const shortcut = app.get<HTMLInputElement>("#shortcut");
    fireEvent.change(shortcut, { target: { value: "Ctrl+Alt+K" } });
    await vi.waitFor(() =>
      expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false),
    );
    fireEvent.click(app.get<HTMLButtonElement>("#save-settings"));
    await vi.waitFor(() =>
      expect(app.backend.setShortcut).toHaveBeenCalledWith("Ctrl+Alt+K"),
    );
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
    expect(app.get("#preview").hidden).toBe(false);
    app.emit(
      snapshot({
        revision: 2,
        windowMode: "locked",
        canLock: true,
        shortcutAvailable: true,
      }),
    );
    expect(app.get(".overlay").dataset.mode).toBe("locked");
    expect(app.get("#settings").hidden).toBe(true);
  });

  it("keeps the caption visible when leaving edit mode", async () => {
    const app = setup(snapshot({ capture: "listening", canLock: true }));
    await app.ready();
    app.emitCaption({
      sessionId: 0,
      segmentId: 1,
      revision: 1,
      mode: "auto",
      text: "Still visible",
      isFinal: true,
      speakers: [],
      language: "en-US",
    });
    app.emit(
      snapshot({
        revision: 1,
        capture: "listening",
        windowMode: "locked",
        canLock: true,
      }),
    );
    expect(app.get(".overlay").dataset.mode).toBe("locked");
    expect(app.get<HTMLElement>("#caption-live").hidden).toBe(false);
    expect(app.get("#caption-live").textContent).toBe("Still visible");
  });

  it("keeps settings open after a rejected save and closes them after a successful retry", async () => {
    const app = setup();
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    await vi.waitFor(() =>
      expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false),
    );
    vi.mocked(app.backend.setShortcut).mockRejectedValueOnce({
      code: "shortcut-unavailable",
    });
    fireEvent.change(app.get<HTMLInputElement>("#shortcut"), {
      target: { value: "Ctrl+Alt+K" },
    });
    fireEvent.click(app.get<HTMLButtonElement>("#save-settings"));
    await vi.waitFor(() => expect(app.get("#error").hidden).toBe(false));
    expect(app.get("#settings").hidden).toBe(false);
    await vi.waitFor(() =>
      expect(app.get<HTMLButtonElement>("#save-settings").disabled).toBe(false),
    );
    fireEvent.change(app.get<HTMLInputElement>("#shortcut"), {
      target: { value: "Ctrl+Alt+K" },
    });
    fireEvent.click(app.get<HTMLButtonElement>("#save-settings"));
    await vi.waitFor(() => expect(app.get("#settings").hidden).toBe(true));
  });

  it("keeps the listening view empty when there is no current caption", async () => {
    const app = setup(snapshot({ capture: "listening" }));
    await app.ready();
    expect(app.get("#caption-preview").hidden).toBe(true);
    expect(app.get("#caption-live").hidden).toBe(true);
    expect(app.root.textContent).not.toContain("Aguardando fala");
    expect(app.root.textContent).not.toContain("Áudio e modelos indisponíveis");
  });

  it("lists output names but selects by device ID", async () => {
    const app = setup();
    vi.mocked(app.backend.getAudioOutputs).mockResolvedValue({
      devices: [
        { id: "speaker-a", name: "Alto-falantes" },
        { id: "speaker-b", name: "Alto-falantes" },
      ],
      selectedId: "speaker-b",
      selectedAvailable: true,
    });
    vi.mocked(app.backend.selectAudioOutput).mockResolvedValue({
      devices: [
        { id: "speaker-a", name: "Alto-falantes" },
        { id: "speaker-b", name: "Alto-falantes" },
      ],
      selectedId: "speaker-a",
      selectedAvailable: true,
    });
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    const select = app.get<HTMLSelectElement>("#audio-output");
    await vi.waitFor(() => expect(select.value).toBe("speaker-b"));
    expect([...select.options].map((option) => option.textContent)).toEqual([
      "Alto-falantes",
      "Alto-falantes",
    ]);
    fireEvent.change(select, { target: { value: "speaker-a" } });
    await vi.waitFor(() =>
      expect(app.backend.selectAudioOutput).toHaveBeenCalledWith("speaker-a"),
    );
    await vi.waitFor(() => expect(select.value).toBe("speaker-a"));
  });

  it("shows a disconnected saved output without switching to another device", async () => {
    const app = setup();
    vi.mocked(app.backend.getAudioOutputs).mockResolvedValue({
      devices: [{ id: "speaker-b", name: "Monitor" }],
      selectedId: "speaker-a",
      selectedAvailable: false,
    });
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    const select = app.get<HTMLSelectElement>("#audio-output");
    await vi.waitFor(() => expect(select.value).toBe("speaker-a"));
    expect(app.get("#audio-output-status").textContent).toContain(
      "desconectada",
    );
    expect(app.backend.selectAudioOutput).not.toHaveBeenCalled();
  });

  it("displays sanitized errors and restores confirmed values after rejection", async () => {
    const app = setup(
      snapshot({
        resources: {
          audioCapture: true,
          transcription: true,
          translation: true,
          speakerLabels: false,
        },
      }),
    );
    await app.ready();
    vi.mocked(app.backend.setMode).mockRejectedValue(
      "<img src=x onerror=alert(1)>",
    );
    fireEvent.click(app.get<HTMLButtonElement>("#language"));
    fireEvent.click(
      app.get<HTMLLIElement>('#language-menu li[data-value="translate-pt-br"]'),
    );
    await vi.waitFor(() => expect(app.get("#error").hidden).toBe(false));
    expect(app.get("#error").querySelector("img")).toBeNull();
    expect(app.get("#error").textContent).toBe(
      "Não foi possível concluir a ação. Tente novamente.",
    );
    expect(app.get("#language-label").textContent).toBe("Transcrever");
  });

  it("releases listeners and ignores events after disposal", async () => {
    const app = setup();
    await app.ready();
    app.overlay.dispose();
    app.emit(snapshot({ revision: 2, mode: "translate-pt-br" }));
    expect(app.root.childElementCount).toBe(0);
    expect(app.release).toHaveBeenCalledOnce();
    expect(app.backend.close).not.toHaveBeenCalled();
  });

  it("starts horizontal resizing from either side only with the primary mouse button", async () => {
    const app = setup();
    await app.ready();
    fireEvent(
      app.get("#resize-right"),
      new MouseEvent("mousedown", { bubbles: true, button: 2 }),
    );
    expect(app.backend.resize).not.toHaveBeenCalled();
    fireEvent(
      app.get("#resize-left"),
      new MouseEvent("mousedown", { bubbles: true, button: 0 }),
    );
    fireEvent(
      app.get("#resize-right"),
      new MouseEvent("mousedown", { bubbles: true, button: 0 }),
    );
    expect(app.backend.resize).toHaveBeenNthCalledWith(1, "West");
    expect(app.backend.resize).toHaveBeenNthCalledWith(2, "East");
  });
});

describe("model download", () => {
  it("offers the download only while the selected engine's model is missing", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.ready();
    const row = app.get("#model-download");
    expect(row.hidden).toBe(false);
    expect(app.get("#model-download-status").textContent).toBe(
      "O modelo deste mecanismo ainda não foi baixado.",
    );
    expect(app.get("#model-download-action").textContent).toBe(
      "Baixar (2.8 GB)",
    );
    app.emit(snapshot({ revision: 1, translationEngine: "opus" }));
    expect(row.hidden).toBe(true);
  });

  it("starts the download, shows progress and lets the user cancel", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#model-download-action"));
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
    fireEvent.click(app.get<HTMLButtonElement>("#model-download-action"));
    expect(app.backend.cancelModelDownload).toHaveBeenCalled();
  });

  it("explains a failure and offers a retry", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.ready();
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
    expect(app.get("#model-download-action").textContent).toBe(
      "Tentar de novo",
    );
    expect(
      app.get<HTMLProgressElement>("#model-download-progress").hidden,
    ).toBe(true);
  });

  it("hides the offer once the installed model arrives", async () => {
    const app = setup(snapshot({ translationEngine: "riva" }));
    await app.ready();
    vi.mocked(app.backend.getModelDownloads).mockResolvedValue([
      {
        model: RIVA_MODEL,
        kind: "translation",
        engine: "riva",
        installed: true,
        downloadable: true,
        downloading: false,
        totalBytes: RIVA_BYTES,
      },
    ]);
    app.emitModelDownload({
      model: RIVA_MODEL,
      phase: "ready",
      receivedBytes: RIVA_BYTES,
      totalBytes: RIVA_BYTES,
      reason: null,
    });
    await vi.waitFor(() =>
      expect(app.get("#model-download").hidden).toBe(true),
    );
  });
});

describe("React lifecycle and interaction", () => {
  it("releases subscriptions that complete after unmount", async () => {
    const app = setup();
    await app.ready();
    let resolve: ((release: () => void) => void) | undefined;
    const release = vi.fn();
    vi.mocked(app.backend.subscribe).mockImplementation(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const view = render(createElement(App, { backend: app.backend }));
    view.unmount();
    await act(async () => {
      resolve?.(release);
    });
    expect(release).toHaveBeenCalledOnce();
  });

  it("keeps appearance drafts across backend events and discards them on cancel", async () => {
    const app = setup();
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    await app.ready();
    fireEvent.change(app.get("#font-size"), { target: { value: "55" } });
    app.emit(snapshot({ revision: 2, capture: "listening" }));
    expect(app.get<HTMLInputElement>("#font-size").value).toBe("55");
    fireEvent.keyDown(app.get("#settings"), { key: "Escape" });
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    expect(app.get<HTMLInputElement>("#font-size").value).toBe("32");
    expect(app.backend.setAppearance).not.toHaveBeenCalled();
  });

  it("closes the language menu with Escape before closing settings", async () => {
    const app = setup();
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#settings-toggle"));
    await app.ready();
    fireEvent.click(app.get<HTMLButtonElement>("#source-language"));
    fireEvent.keyDown(app.get("#source-language"), { key: "Escape" });
    expect(app.get("#source-language-menu").hidden).toBe(true);
    expect(app.get("#settings").hidden).toBe(false);
    expect(document.activeElement).toBe(app.get("#source-language"));
  });
});


it("cleans up every state subscription under React StrictMode", async () => {
  const app = setup();
  await app.ready();
  const before = app.release.mock.calls.length;
  const view = render(createElement(StrictMode, null, createElement(App, { backend: app.backend })));
  await app.ready();
  expect(app.release.mock.calls.length).toBe(before + 1);
  view.unmount();
  expect(app.release.mock.calls.length).toBe(before + 2);
});
