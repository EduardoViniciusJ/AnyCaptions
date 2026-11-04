import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render } from "@testing-library/react";
import { createElement } from "react";
import { App } from "./app/App";
import type {
  AppSnapshot,
  Backend,
  CaptionUpdate,
  ModelDownloadState,
  ModelDownloadUpdate,
} from "./contracts";

const RIVA_MODEL = "riva-translate-4b-instruct-v2-q4_k_m";
const DIARIZATION_MODEL = "nemotron-3-diarization-q8_0";
const DIARIZATION_BYTES = 107_012_128;

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
      audioCapture: true,
      transcription: true,
      translation: false,
      speakerLabels: false,
    },
    canStart: true,
    canLock: false,
    shortcut: "Ctrl+Alt+L",
    shortcutAvailable: false,
    error: null,
    version: "0.1.1",
    updateCheck: false,
    ...overrides,
  };
}

const downloadStates = (diarizationInstalled: boolean): ModelDownloadState[] => [
  {
    model: RIVA_MODEL,
    kind: "translation",
    engine: "riva",
    installed: false,
    downloadable: true,
    downloading: false,
    totalBytes: 2_759_302_368,
  },
  {
    model: DIARIZATION_MODEL,
    kind: "diarization",
    engine: null,
    installed: diarizationInstalled,
    downloadable: true,
    downloading: false,
    totalBytes: DIARIZATION_BYTES,
  },
];

const disposers: (() => void)[] = [];
afterEach(() => {
  disposers.splice(0).forEach((dispose) => dispose());
  document.body.replaceChildren();
});

function setup(initial = snapshot(), diarizationInstalled = false) {
  const root = document.createElement("main");
  document.body.append(root);
  let listener: ((state: AppSnapshot) => void) | undefined;
  let captionListener: ((caption: CaptionUpdate) => void) | undefined;
  const downloadListeners: ((update: ModelDownloadUpdate) => void)[] = [];
  const selection = { devices: [], selectedId: null, selectedAvailable: false };
  const backend: Backend = {
    getState: vi.fn().mockResolvedValue(initial),
    subscribe: vi.fn(async (callback) => {
      listener = callback;
      return vi.fn();
    }),
    subscribeCaptions: vi.fn(async (callback) => {
      captionListener = callback;
      return vi.fn();
    }),
    setMode: vi.fn(async (mode) => snapshot({ mode, revision: 1, sessionId: 1 })),
    setTranslationEngine: vi.fn(async (translationEngine) =>
      snapshot({ translationEngine, revision: 1, sessionId: 1 }),
    ),
    setSourceLanguage: vi.fn(async (sourceLanguage) =>
      snapshot({ sourceLanguage, revision: 1, sessionId: 1 }),
    ),
    setSpeakerLabels: vi.fn(async (speakerLabels) =>
      snapshot({ speakerLabels, revision: 1, sessionId: 1 }),
    ),
    setUpdateCheck: vi.fn(async (updateCheck) =>
      snapshot({ updateCheck, revision: 1 }),
    ),
    checkForUpdates: vi.fn().mockResolvedValue({ current: "0.1.1", latest: "0.1.1", available: false }),
    openReleasesPage: vi.fn().mockResolvedValue(undefined),
    setUiLanguage: vi.fn(async (uiLanguage) => snapshot({ uiLanguage, revision: 1 })),
    start: vi.fn().mockResolvedValue(snapshot({ capture: "preparing", revision: 1 })),
    stop: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    setAppearance: vi.fn(async (appearance) => snapshot({ appearance, revision: 1 })),
    setWindowMode: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    setShortcut: vi.fn().mockResolvedValue(snapshot({ revision: 1 })),
    getAudioOutputs: vi.fn().mockResolvedValue(selection),
    selectAudioOutput: vi.fn().mockResolvedValue(selection),
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
      inputs: selection,
    }),
    setDictation: vi.fn(),
    getModelDownloads: vi.fn().mockResolvedValue(downloadStates(diarizationInstalled)),
    downloadModel: vi.fn().mockResolvedValue(undefined),
    cancelModelDownload: vi.fn().mockResolvedValue(undefined),
    subscribe_: undefined,
    subscribeDictation: vi.fn(async () => vi.fn()),
    subscribeModelDownloads: vi.fn(async (callback) => {
      downloadListeners.push(callback);
      return vi.fn();
    }),
    resize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    openProjectPage: vi.fn().mockResolvedValue(undefined),
  } as unknown as Backend;
  const view = render(createElement(App, { backend }), { container: root });
  disposers.push(() => view.unmount());
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
  const openSettings = async () => {
    fireEvent.click(get("#settings-toggle"));
    await ready();
  };
  return {
    root,
    backend,
    ready,
    get,
    openSettings,
    emit: (state: AppSnapshot) => act(() => listener?.(state)),
    emitCaption: (caption: CaptionUpdate) => act(() => captionListener?.(caption)),
    emitModelDownload: (update: ModelDownloadUpdate) =>
      act(() => downloadListeners.forEach((callback) => callback(update))),
  };
}

const caption = (
  overrides: Partial<CaptionUpdate> & Pick<CaptionUpdate, "text" | "speakers">,
): CaptionUpdate => ({
  sessionId: 0,
  segmentId: 1,
  revision: 1,
  mode: "auto",
  isFinal: true,
  language: "en-US",
  ...overrides,
});

describe("speaker labels", () => {
  it("is off by default and turns on through Rust from the settings", async () => {
    const app = setup();
    await app.ready();
    await app.openSettings();
    const toggle = app.get<HTMLInputElement>("#speaker-labels");
    expect(toggle.checked).toBe(false);
    expect(app.get("#speaker-labels-hint").textContent).toContain("Pessoa 1");
    expect(app.get("#speaker-labels-hint").textContent).toContain(
      "não reconhece identidades",
    );
    fireEvent.click(toggle);
    expect(app.backend.setSpeakerLabels).toHaveBeenCalledWith(true);
    await app.ready();
    expect(app.get<HTMLInputElement>("#speaker-labels").checked).toBe(true);
    // Choosing the same value again does not call the backend.
    await app.emit(snapshot({ revision: 2, speakerLabels: true }));
    expect(app.backend.setSpeakerLabels).toHaveBeenCalledTimes(1);
  });

  it("tells the user when labels are on but the diarizer is unavailable", async () => {
    const app = setup(snapshot({ speakerLabels: true }));
    await app.ready();
    const notice = app.get("#notice");
    expect(notice.hidden).toBe(false);
    expect(notice.textContent).toBe(
      "Identificação de falantes indisponível; as legendas seguem sem rótulos.",
    );
    await app.emit(
      snapshot({
        revision: 1,
        speakerLabels: true,
        resources: { ...snapshot().resources, speakerLabels: true },
      }),
    );
    expect(app.get("#notice").hidden).toBe(true);
    await app.emit(snapshot({ revision: 2, speakerLabels: false }));
    expect(app.get("#notice").hidden).toBe(true);
  });

  it("offers the diarization model download on its own row", async () => {
    const app = setup();
    await app.ready();
    await app.openSettings();
    const row = app.get("#diarization-download");
    expect(row.hidden).toBe(false);
    expect(app.get("#diarization-download-status").textContent).toBe(
      "O modelo de identificação de falantes ainda não foi baixado.",
    );
    expect(app.get("#diarization-download-action").textContent).toBe("Baixar (107 MB)");
    fireEvent.click(app.get("#diarization-download-action"));
    expect(app.backend.downloadModel).toHaveBeenCalledWith(DIARIZATION_MODEL);
    await app.emitModelDownload({
      model: DIARIZATION_MODEL,
      phase: "downloading",
      receivedBytes: DIARIZATION_BYTES / 4,
      totalBytes: DIARIZATION_BYTES,
      reason: null,
    });
    expect(app.get("#diarization-download-status").textContent).toBe("Baixando… 25%");
    expect(app.get<HTMLProgressElement>("#diarization-download-progress").hidden).toBe(false);
    // The translation row (Riva, hidden under OPUS) is untouched by this event.
    expect(app.get("#model-download").hidden).toBe(true);
    await app.emitModelDownload({
      model: DIARIZATION_MODEL,
      phase: "failed",
      receivedBytes: 0,
      totalBytes: null,
      reason: "corrupted",
    });
    expect(app.get("#diarization-download-status").textContent).toBe(
      "O arquivo baixado não passou na verificação. Tente de novo.",
    );
    expect(app.get("#diarization-download-action").textContent).toBe("Tentar de novo");
  });

  it("hides the download once the model is installed", async () => {
    const app = setup(snapshot(), true);
    await app.ready();
    await app.openSettings();
    expect(app.get("#diarization-download").hidden).toBe(true);
    expect(app.get("#diarization-download-progress").hidden).toBe(true);
  });

  it("shows generic labels in the live caption and keeps unlabeled captions untouched", async () => {
    const app = setup(
      snapshot({
        capture: "listening",
        speakerLabels: true,
        resources: { ...snapshot().resources, speakerLabels: true },
      }),
    );
    await app.ready();
    await app.emitCaption(
      caption({
        text: "Did you finish? Yes, just now.",
        speakers: [
          { speaker: 1, text: "Did you finish?" },
          { speaker: 2, text: "Yes, just now." },
        ],
      }),
    );
    const labels = [...app.root.querySelectorAll<HTMLElement>(".caption-speaker")];
    expect(labels.map((label) => label.textContent?.trim())).toEqual([
      "Pessoa 1:",
      "Pessoa 2:",
    ]);
    expect(labels.map((label) => label.dataset.speaker)).toEqual(["1", "2"]);
    expect(app.get("#caption-live").textContent).toBe(
      "Pessoa 1: Did you finish? Pessoa 2: Yes, just now.",
    );
    // A later caption with the speaker still unknown adds words without a label.
    await app.emitCaption(
      caption({
        segmentId: 2,
        isFinal: false,
        language: null,
        text: "and",
        speakers: [{ speaker: 0, text: "and" }],
      }),
    );
    expect(app.root.querySelectorAll(".caption-speaker").length).toBe(2);
    expect(app.get("#caption-live").textContent).toContain("Yes, just now. and");

    // Labels off: the wire carries no speakers and nothing is labeled.
    const plain = setup(snapshot({ capture: "listening" }));
    await plain.ready();
    await plain.emitCaption(caption({ text: "Plain caption", speakers: [] }));
    expect(plain.root.querySelector(".caption-speaker")).toBeNull();
    expect(plain.get("#caption-live").textContent).toBe("Plain caption");
  });

  it("keeps the label with each translated sentence", async () => {
    const app = setup(
      snapshot({
        mode: "translate-pt-br",
        capture: "listening",
        speakerLabels: true,
        resources: { ...snapshot().resources, translation: true, speakerLabels: true },
      }),
    );
    await app.ready();
    const translated = (segmentId: number, speaker: number, text: string) =>
      caption({
        segmentId,
        mode: "translate-pt-br",
        language: "pt-br",
        text,
        speakers: [{ speaker, text }],
      });
    await app.emitCaption(translated(1, 1, "Você conseguiu terminar?"));
    await app.emitCaption(translated(2, 2, "Sim, terminei agora."));
    await app.emitCaption(translated(3, 2, "E funciona bem."));
    expect(app.get("#caption-live").textContent).toBe(
      "Pessoa 1: Você conseguiu terminar? Pessoa 2: Sim, terminei agora. E funciona bem.",
    );
    expect(app.root.querySelectorAll(".caption-sentence").length).toBe(3);
  });
});
