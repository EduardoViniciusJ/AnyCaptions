import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render } from "@testing-library/react";
import { createElement } from "react";
import { App } from "./app/App";
import { CAPTION_FONTS, captionFontFamily, sameAppearance } from "./appearance";
import type { AppSnapshot, Backend, UpdateCheck } from "./contracts";

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

const disposers: (() => void)[] = [];
afterEach(() => {
  disposers.splice(0).forEach((dispose) => dispose());
  document.body.replaceChildren();
});

function setup(initial = snapshot(), update: UpdateCheck | Error = { current: "0.1.1", latest: "0.1.1", available: false, installable: false }) {
  const root = document.createElement("main");
  document.body.append(root);
  let listener: ((state: AppSnapshot) => void) | undefined;
  const selection = { devices: [], selectedId: null, selectedAvailable: false };
  const checkForUpdates = vi.fn(() =>
    update instanceof Error ? Promise.reject(update) : Promise.resolve(update),
  );
  const backend = {
    getState: vi.fn().mockResolvedValue(initial),
    subscribe: vi.fn(async (callback: (state: AppSnapshot) => void) => {
      listener = callback;
      return vi.fn();
    }),
    subscribeCaptions: vi.fn(async () => vi.fn()),
    subscribeDictation: vi.fn(async () => vi.fn()),
    subscribeModelDownloads: vi.fn(async () => vi.fn()),
    setMode: vi.fn(),
    setTranslationEngine: vi.fn(),
    setSourceLanguage: vi.fn(),
    setSpeakerLabels: vi.fn(),
    setUpdateCheck: vi.fn(async (updateCheck: boolean) => snapshot({ updateCheck, revision: 1 })),
    checkForUpdates,
    openReleasesPage: vi.fn().mockResolvedValue(undefined),
    setUiLanguage: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    setAppearance: vi.fn(async (appearance) => snapshot({ appearance, revision: 1 })),
    setWindowMode: vi.fn(),
    setShortcut: vi.fn(),
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
    getModelDownloads: vi.fn().mockResolvedValue([]),
    downloadModel: vi.fn(),
    cancelModelDownload: vi.fn(),
    resize: vi.fn(),
    close: vi.fn(),
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
  };
}

describe("caption fonts", () => {
  it("maps every catalog font to a CSS family and falls back to Inter", () => {
    expect(CAPTION_FONTS.map((font) => font.id)).toEqual([
      "inter",
      "fira-code",
      "segoe-ui",
      "arial",
      "verdana",
      "tahoma",
      "trebuchet-ms",
      "georgia",
      "consolas",
    ]);
    expect(captionFontFamily("fira-code")).toContain("Fira Code");
    expect(captionFontFamily("unknown")).toContain("Inter");
    const base = snapshot().appearance;
    expect(sameAppearance(base, { ...base, font: "georgia" })).toBe(false);
  });

  it("offers the fonts in the settings and saves the chosen one with the appearance", async () => {
    const app = setup();
    await app.ready();
    await app.openSettings();
    const select = app.get<HTMLSelectElement>("#caption-font");
    expect([...select.options].map((option) => option.value)).toEqual(CAPTION_FONTS.map((font) => font.id));
    expect(select.value).toBe("inter");
    fireEvent.change(select, { target: { value: "consolas" } });
    // The preview follows the draft before saving.
    expect(app.get("#appearance-preview").style.getPropertyValue("--caption-font")).toContain("Consolas");
    expect(app.backend.setAppearance).not.toHaveBeenCalled();
    fireEvent.click(app.get("#save-settings"));
    await app.ready();
    expect(app.backend.setAppearance).toHaveBeenCalledWith({ ...snapshot().appearance, font: "consolas" });
    // The live caption element picks the saved font up from the snapshot.
    expect(app.get("#caption-live").style.getPropertyValue("--caption-font")).toContain("Consolas");
  });
});

describe("updates", () => {
  it("shows the version and the developer credit with a GitHub link", async () => {
    const app = setup();
    await app.ready();
    await app.openSettings();
    expect(app.get("#app-version").textContent).toBe("v0.1.1");
    const credit = app.get("#developer-credit");
    expect(credit.querySelector("svg")).not.toBeNull();
    expect(credit.textContent).toContain("EduardoViniciusJ");
    expect(credit.textContent).toContain("desenvolvido por");
    fireEvent.click(credit);
    expect(app.backend.openProjectPage).toHaveBeenCalled();
  });

  it("checks on demand and offers the download when a newer release exists", async () => {
    const app = setup(snapshot(), { current: "0.1.1", latest: "0.2.0", available: true, installable: false });
    await app.ready();
    // Off by default: nothing is checked at startup.
    expect(app.backend.checkForUpdates).not.toHaveBeenCalled();
    expect(app.root.querySelector("#update-notice")).toBeNull();
    await app.openSettings();
    expect(app.get<HTMLInputElement>("#update-check").checked).toBe(false);
    expect(app.get("#update-status").textContent).toContain("GitHub");
    fireEvent.click(app.get("#update-check-now"));
    await app.ready();
    expect(app.backend.checkForUpdates).toHaveBeenCalledTimes(1);
    expect(app.get("#update-status").textContent).toBe("Nova versão 0.2.0 disponível.");
    fireEvent.click(app.get("#update-download"));
    expect(app.backend.openReleasesPage).toHaveBeenCalled();
    // Turning the startup check on goes through Rust.
    fireEvent.click(app.get("#update-check"));
    expect(app.backend.setUpdateCheck).toHaveBeenCalledWith(true);
  });

  it("reports an up-to-date app and a failed check without breaking anything", async () => {
    const app = setup();
    await app.ready();
    await app.openSettings();
    fireEvent.click(app.get("#update-check-now"));
    await app.ready();
    expect(app.get("#update-status").textContent).toBe("Você já tem a versão mais recente (0.1.1).");
    expect(app.root.querySelector("#update-download")).toBeNull();

    const failing = setup(snapshot(), new Error("offline"));
    await failing.ready();
    await failing.openSettings();
    fireEvent.click(failing.get("#update-check-now"));
    await failing.ready();
    expect(failing.get("#update-status").textContent).toBe("Não foi possível verificar agora. Tente de novo mais tarde.");
  });

  it("checks once at startup when enabled and shows a dismissible notice", async () => {
    const app = setup(snapshot({ updateCheck: true }), { current: "0.1.1", latest: "0.3.0", available: true, installable: false });
    await app.ready();
    expect(app.backend.checkForUpdates).toHaveBeenCalledTimes(1);
    const notice = app.get("#update-notice");
    expect(notice.textContent).toContain("Nova versão 0.3.0 disponível.");
    fireEvent.click(app.get("#update-notice-download"));
    expect(app.backend.openReleasesPage).toHaveBeenCalled();
    // A later snapshot does not trigger another request.
    await app.emit(snapshot({ revision: 2, updateCheck: true }));
    expect(app.backend.checkForUpdates).toHaveBeenCalledTimes(1);
    fireEvent.click(app.get("#update-notice-dismiss"));
    expect(app.root.querySelector("#update-notice")).toBeNull();
  });
});

describe("other languages hint", () => {
  it("points OPUS users to the Riva download and disappears with Riva", async () => {
    const app = setup();
    await app.ready();
    const footer = app.get("#language-footer");
    expect(footer.textContent).toContain("Riva");
    expect(footer.closest('[role="option"]')).toBeNull();
    fireEvent.click(app.get("#language"));
    fireEvent.click(footer);
    await app.ready();
    expect(app.get("#settings").hidden).toBe(false);
    await app.emit(snapshot({ revision: 1, translationEngine: "riva" }));
    expect(app.root.querySelector("#language-footer")).toBeNull();
  });
});
