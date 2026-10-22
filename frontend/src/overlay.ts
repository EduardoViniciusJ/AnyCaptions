import { TRANSLATION_TARGETS, flagUrl } from "./languages";
import { UI_LANGUAGES, localeTag, setLocale, t, type MessageKey } from "./i18n";
import type {
  AppSnapshot,
  Appearance,
  Backend,
  CaptionMode,
  ResizeDirection,
  OutputSelection,
} from "./contracts";
import {
  DEFAULT_APPEARANCE,
  applyAppearance,
  normalizeCaptionColor,
  sameAppearance,
} from "./appearance";
import { CaptionPresenter } from "./captions";

const icons = {
  play: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 5 11 7-11 7Z"/></svg>',
  lock: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0"/></svg>',
  settings: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/></svg>',
  close: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>',
  // Globo: detecção automática de idioma.
  globe:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.6 3.8 5.7 3.8 9S14.5 18.4 12 21c-2.5-2.6-3.8-5.7-3.8-9S9.5 5.6 12 3Z"/></svg>',
};

function errorMessage(error: unknown): string {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : null;
  switch (code) {
    case "capture-unavailable": return t("errCapture");
    case "transcription-unavailable": return t("errTranscription");
    case "translation-unavailable": return t("errTranslation");
    case "invalid-font-size": return t("errFontSize");
    case "recovery-unavailable": return t("errRecovery");
    case "invalid-shortcut": return t("errShortcutFormat");
    case "shortcut-unavailable": return t("errShortcutTaken");
    case "not-found": return t("errOutputGone");
    case "invalid-id": return t("errOutputInvalid");
    default: return t("errGeneric");
  }
}

export function createOverlay(root: HTMLElement, backend: Backend) {
  // Only this trusted, static template uses HTML. All variable text uses textContent.
  root.innerHTML = `
    <section class="overlay" data-mode="edit" data-i18n-aria="captionWindow" aria-label="Janela de legendas">
      <header class="toolbar">
        <div class="brand" data-tauri-drag-region data-i18n-title="dragMove" title="Arraste para mover a janela">
          <span class="brand-mark" data-tauri-drag-region aria-hidden="true">cc</span>
          <span class="brand-name" data-tauri-drag-region>AnyCaptions</span>
        </div>
        <div class="drag-region" data-tauri-drag-region data-i18n-title="dragMove" title="Arraste para mover a janela"></div>
        <div class="controls" data-i18n-aria="captionControls" aria-label="Controles de legendas">
          <button id="capture" class="capture-button" disabled>${icons.play}<span id="capture-label">Iniciar</span></button>
          <div class="language-picker">
            <button id="language" type="button" disabled aria-haspopup="listbox" aria-expanded="false" data-i18n-aria="captionLanguage" aria-label="Idioma das legendas"><span id="language-label">Auto</span></button>
            <ul id="language-menu" role="listbox" data-i18n-aria="captionLanguage" aria-label="Idioma das legendas" hidden></ul>
          </div>
          <button id="settings-toggle" class="icon-button" data-i18n-aria="settings" aria-label="Configurações" data-i18n-title="settings" title="Configurações" aria-expanded="false" aria-controls="settings">${icons.settings}</button>
          <button id="lock" class="icon-button" data-i18n-aria="lockWindow" aria-label="Bloquear janela" data-i18n-title="lockHint" title="O bloqueio precisa de atalho ou bandeja disponíveis" disabled>${icons.lock}</button>
        </div>
          <button id="close" class="icon-button close-button" data-i18n-aria="closeToTray" aria-label="Recolher à bandeja" data-i18n-title="closeToTray" title="Recolher à bandeja">${icons.close}</button>
      </header>
      <div class="content">
        <div id="preview" class="preview">
          <p id="caption-preview" class="caption" data-i18n="captionPlaceholder">Suas legendas aparecem aqui</p>
          <p id="caption-live" class="caption" aria-live="polite" hidden></p>
        </div>
        <section id="settings" class="settings" data-i18n-aria="settingsAria" aria-label="Configurações de legendas" hidden>
          <h2 data-i18n="settings">Configurações</h2>
          <div class="appearance-preview" data-i18n-aria="previewAria" aria-label="Prévia da legenda">
            <span class="appearance-preview-label" data-i18n="preview">Prévia</span>
            <p id="appearance-preview" class="caption appearance-example"><span data-i18n="previewFinal">Texto </span><span class="caption-partial" data-i18n="previewPartial">fica claro</span></p>
          </div>
          <div class="setting-row">
            <label for="font-size"><span data-i18n="fontSize">Tamanho</span> <output id="font-value" for="font-size">32 px</output></label>
            <input id="font-size" type="range" min="16" max="80" value="32" step="1" disabled />
          </div>
          <div class="setting-row">
            <label for="font-color" data-i18n="confirmedText">Texto confirmado</label>
            <div class="color-control">
              <input id="font-color" type="color" value="#ffffff" data-i18n-aria="pickConfirmedColor" aria-label="Escolher cor do texto confirmado" disabled />
              <input id="font-color-value" type="text" value="#FFFFFF" maxlength="7" spellcheck="false" data-i18n-aria="confirmedColorHex" aria-label="Cor hexadecimal do texto confirmado" disabled />
            </div>
          </div>
          <div class="setting-row">
            <label for="partial-color" data-i18n="recentWords">Palavras recentes</label>
            <div class="color-control">
              <input id="partial-color" type="color" value="#b8b8b8" data-i18n-aria="pickRecentColor" aria-label="Escolher cor das palavras recentes" disabled />
              <input id="partial-color-value" type="text" value="#B8B8B8" maxlength="7" spellcheck="false" data-i18n-aria="recentColorHex" aria-label="Cor hexadecimal das palavras recentes" disabled />
            </div>
          </div>
          <div class="setting-row">
            <label for="ui-language" data-i18n="uiLanguage">Idioma da interface</label>
            <div class="language-picker">
              <button id="ui-language" type="button" aria-haspopup="listbox" aria-expanded="false"><span id="ui-language-label"></span></button>
              <ul id="ui-language-menu" role="listbox" data-i18n-aria="uiLanguage" aria-label="Idioma da interface" hidden></ul>
            </div>
          </div>
          <div class="setting-row">
            <label for="audio-output" data-i18n="audioOutput">Saída de áudio</label>
            <select id="audio-output" disabled><option>Nenhuma saída disponível</option></select>
          </div>
          <p id="audio-output-status" class="muted" hidden></p>
          <div class="setting-row">
            <label for="shortcut" data-i18n="lockShortcut">Atalho de bloqueio</label>
            <div class="setting-actions">
              <input id="shortcut" type="text" value="Ctrl+Alt+L" aria-describedby="shortcut-status" maxlength="16" spellcheck="false" />
            </div>
          </div>
          <p id="shortcut-status" class="muted" hidden></p>
          <div class="settings-footer">
            <button id="reset-appearance" class="secondary-button" type="button" data-i18n="restoreDefault">Restaurar padrão</button>
            <button id="save-settings" type="button" data-i18n="save">Salvar</button>
          </div>
        </section>
      </div>
      <p id="notice" class="notice" role="status" hidden></p>
      <p id="error" class="error" role="alert" hidden></p>
      <div id="resize-left" class="resize-grip resize-grip-left" data-i18n-title="dragWidth" title="Arraste para ajustar a largura" aria-hidden="true"></div>
      <div id="resize-right" class="resize-grip resize-grip-right" data-i18n-title="dragWidth" title="Arraste para ajustar a largura" aria-hidden="true"></div>
      <div id="resize-bottom" class="resize-grip resize-grip-bottom" data-i18n-title="dragHeight" title="Arraste para ajustar a altura" aria-hidden="true"></div>
    </section>`;

  function get<T extends HTMLElement>(selector: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`Missing overlay element: ${selector}`);
    return element;
  }

  const overlay = get<HTMLElement>(".overlay");
  const capture = get<HTMLButtonElement>("#capture");
  const captureLabel = get<HTMLElement>("#capture-label");
  const language = get<HTMLButtonElement>("#language");
  const languageLabel = get<HTMLElement>("#language-label");
  const languageMenu = get<HTMLUListElement>("#language-menu");
  const languageItem = (mode: CaptionMode, label: string, name: string, country?: string) => {
    const item = document.createElement("li");
    item.role = "option";
    item.dataset.value = mode;
    item.dataset.name = name;
    item.title = name;
    const flag = country && flagUrl(country);
    if (flag) {
      const image = document.createElement("img");
      image.className = "flag";
      image.src = flag;
      image.alt = "";
      item.append(image);
    } else {
      // Sem país: ícone de globo (detecção automática).
      const globe = document.createElement("span");
      globe.className = "flag flag-globe";
      globe.innerHTML = icons.globe;
      item.append(globe);
    }
    const text = document.createElement("span");
    text.textContent = label;
    item.append(text);
    languageMenu.append(item);
    return item;
  };
  languageItem("auto", t("autoMode"), t("autoModeHint"));
  for (const target of TRANSLATION_TARGETS) {
    languageItem(target.mode, `${t("translateTo")} ${target.short}`, target.name, target.country);
  }
  const languageItems = [...languageMenu.querySelectorAll<HTMLLIElement>("li")];
  const toggleLanguageMenu = (open: boolean) => {
    languageMenu.hidden = !open;
    language.setAttribute("aria-expanded", String(open));
    if (open) {
      const selected = languageItems.find((item) => item.dataset.value === state?.mode);
      selected?.scrollIntoView({ block: "nearest" });
    }
  };
  const renderLanguageButton = (mode: CaptionMode) => {
    const item = languageItems.find((entry) => entry.dataset.value === mode);
    languageLabel.textContent = item?.querySelector("span:not(.flag)")?.textContent ?? "Auto";
    language.title = item?.dataset.name ?? "";
    language.querySelector(".flag")?.remove();
    const flag = item?.querySelector(".flag");
    if (flag) language.prepend(flag.cloneNode(true));
  };

  // Seletor do idioma da interface (configurações), com as mesmas bandeiras.
  const uiLanguage = get<HTMLButtonElement>("#ui-language");
  const uiLanguageLabel = get<HTMLElement>("#ui-language-label");
  const uiLanguageMenu = get<HTMLUListElement>("#ui-language-menu");
  for (const option of UI_LANGUAGES) {
    const item = document.createElement("li");
    item.role = "option";
    item.dataset.value = option.tag;
    item.title = option.name;
    const flag = flagUrl(option.country);
    if (flag) {
      const image = document.createElement("img");
      image.className = "flag";
      image.src = flag;
      image.alt = "";
      item.append(image);
    }
    const text = document.createElement("span");
    text.textContent = option.name;
    item.append(text);
    uiLanguageMenu.append(item);
  }
  const uiLanguageItems = [...uiLanguageMenu.querySelectorAll<HTMLLIElement>("li")];
  const toggleUiLanguageMenu = (open: boolean) => {
    uiLanguageMenu.hidden = !open;
    uiLanguage.setAttribute("aria-expanded", String(open));
  };
  const renderUiLanguageButton = () => {
    const item = uiLanguageItems.find((entry) => entry.dataset.value === localeTag());
    uiLanguageLabel.textContent = item?.querySelector("span:not(.flag)")?.textContent ?? "";
    uiLanguage.querySelector(".flag")?.remove();
    const flag = item?.querySelector(".flag");
    if (flag) uiLanguage.prepend(flag.cloneNode(true));
    for (const entry of uiLanguageItems) {
      entry.setAttribute("aria-selected", String(entry.dataset.value === localeTag()));
    }
  };

  /** Reaplica todos os textos estáticos no idioma atual da interface. */
  const applyTexts = () => {
    for (const element of root.querySelectorAll<HTMLElement>("[data-i18n]")) {
      element.textContent = t(element.dataset.i18n as MessageKey);
    }
    for (const element of root.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
      element.title = t(element.dataset.i18nTitle as MessageKey);
    }
    for (const element of root.querySelectorAll<HTMLElement>("[data-i18n-aria]")) {
      element.setAttribute("aria-label", t(element.dataset.i18nAria as MessageKey));
    }
    for (const item of languageItems) {
      const text = item.querySelector("span:not(.flag)");
      if (!text) continue;
      if (item.dataset.value === "auto") {
        text.textContent = t("autoMode");
        item.dataset.name = t("autoModeHint");
        item.title = t("autoModeHint");
      } else {
        const target = TRANSLATION_TARGETS.find((entry) => entry.mode === item.dataset.value);
        if (target) text.textContent = `${t("translateTo")} ${target.short}`;
      }
    }
    renderUiLanguageButton();
  };
  const lock = get<HTMLButtonElement>("#lock");
  const settingsToggle = get<HTMLButtonElement>("#settings-toggle");
  const settings = get<HTMLElement>("#settings");
  const preview = get<HTMLElement>("#preview");
  const caption = get<HTMLElement>("#caption-preview");
  const live = get<HTMLElement>("#caption-live");
  const presenter = new CaptionPresenter(live, undefined, () => {
    caption.hidden = !live.hidden || state?.capture !== "stopped";
  });
  const fontSize = get<HTMLInputElement>("#font-size");
  const fontColor = get<HTMLInputElement>("#font-color");
  const fontColorValue = get<HTMLInputElement>("#font-color-value");
  const partialColor = get<HTMLInputElement>("#partial-color");
  const partialColorValue = get<HTMLInputElement>("#partial-color-value");
  const appearancePreview = get<HTMLElement>("#appearance-preview");
  const resetAppearance = get<HTMLButtonElement>("#reset-appearance");
  const saveSettings = get<HTMLButtonElement>("#save-settings");
  const shortcut = get<HTMLInputElement>("#shortcut");
  const audioOutput = get<HTMLSelectElement>("#audio-output");
  const error = get<HTMLElement>("#error");
  const listeners = new AbortController();
  let state: AppSnapshot | undefined;
  let busy = false;
  let disposed = false;
  let unsubscribe: (() => void) | undefined;
  let unsubscribeCaptions: (() => void) | undefined;
  let previousCapture: AppSnapshot["capture"] | undefined;
  let outputs: OutputSelection | undefined;
  let outputBusy = false;
  let appearanceDraft: Appearance | undefined;
  let appearanceValid = true;

  function setAppearanceInputs(appearance: Appearance) {
    appearanceDraft = { ...appearance };
    fontSize.value = String(appearance.fontSize);
    fontColor.value = appearance.color;
    fontColorValue.value = appearance.color;
    partialColor.value = appearance.partialColor;
    partialColorValue.value = appearance.partialColor;
    fontColorValue.removeAttribute("aria-invalid");
    partialColorValue.removeAttribute("aria-invalid");
    appearanceValid = true;
    renderAppearancePreview();
    updateSettingsAvailability();
  }

  function renderAppearancePreview() {
    if (!appearanceDraft) return;
    applyAppearance(appearancePreview, appearanceDraft);
    get("#font-value").textContent = `${appearanceDraft.fontSize} px`;
  }

  function updateSettingsAvailability() {
    const disabled = busy || outputBusy;
    fontSize.disabled = disabled;
    fontColor.disabled = disabled;
    fontColorValue.disabled = disabled;
    partialColor.disabled = disabled;
    partialColorValue.disabled = disabled;
    resetAppearance.disabled = disabled;
    shortcut.disabled = disabled;
    saveSettings.disabled = disabled || !appearanceValid;
  }

  function updateDraftColor(
    value: string,
    textInput: HTMLInputElement,
    picker: HTMLInputElement,
    key: "color" | "partialColor",
  ) {
    const normalized = normalizeCaptionColor(value);
    if (normalized) textInput.removeAttribute("aria-invalid");
    else textInput.setAttribute("aria-invalid", "true");
    appearanceValid = Boolean(
      normalizeCaptionColor(fontColorValue.value)
      && normalizeCaptionColor(partialColorValue.value),
    );
    if (normalized && appearanceDraft) {
      appearanceDraft[key] = normalized;
      picker.value = normalized;
      renderAppearancePreview();
    }
    updateSettingsAvailability();
  }

  function renderOutputs() {
    audioOutput.replaceChildren();
    if (!outputs) {
      audioOutput.add(new Option(t("checkingOutputs"), ""));
      audioOutput.disabled = true;
      return;
    }
    for (const device of outputs.devices) audioOutput.add(new Option(device.name, device.id));
    if (outputs.devices.length && !outputs.selectedId) audioOutput.add(new Option(t("selectOutput"), ""), 0);
    if (outputs.selectedId && !outputs.selectedAvailable) {
      audioOutput.add(new Option(t("savedOutputUnavailable"), outputs.selectedId));
    }
    if (!outputs.devices.length) audioOutput.add(new Option(t("noOutputs"), ""));
    audioOutput.value = outputs.selectedId ?? "";
    audioOutput.disabled = busy || outputBusy || !outputs.devices.length;
    const outputStatus = get("#audio-output-status");
    outputStatus.textContent = outputs.selectedId && !outputs.selectedAvailable
      ? t("savedOutputDisconnected")
      : outputs.devices.length ? "" : t("noActiveOutput");
    outputStatus.hidden = !outputStatus.textContent;
    updateSettingsAvailability();
  }

  async function loadOutputs() {
    if (outputBusy || disposed) return;
    outputBusy = true;
    renderOutputs();
    try { outputs = await backend.getAudioOutputs(); }
    catch { showError(t("errOutputsList")); }
    finally { outputBusy = false; renderOutputs(); }
  }

  function render() {
    if (!state || disposed) return;
    overlay.dataset.mode = state.windowMode;
    presenter.setContext(state.sessionId, state.mode);
    if (state.capture === "stopped" && previousCapture && previousCapture !== "stopped") presenter.clear();
    previousCapture = state.capture;
    caption.hidden = !live.hidden || state.capture !== "stopped";
    if (state.windowMode === "locked" && !settings.hidden) toggleSettings(false);
    if (setLocale(state.uiLanguage)) applyTexts();
    renderLanguageButton(state.mode);
    language.disabled = busy;
    uiLanguage.disabled = busy;
    if (busy || state.windowMode === "locked") {
      toggleLanguageMenu(false);
      toggleUiLanguageMenu(false);
    }
    for (const item of languageItems) {
      const translate = item.dataset.value?.startsWith("translate-") ?? false;
      const unavailable = translate && !state.resources.translation;
      item.setAttribute("aria-disabled", String(unavailable));
      item.setAttribute("aria-selected", String(item.dataset.value === state.mode));
      item.title = unavailable
        ? `${item.dataset.name ?? ""} — ${t("translationMissing")}`
        : (item.dataset.name ?? "");
    }
    if (settings.hidden || !appearanceDraft) setAppearanceInputs(state.appearance);
    applyAppearance(caption, state.appearance);
    applyAppearance(live, state.appearance);
    presenter.refreshLayout();
    const stopped = state.capture === "stopped";
    capture.disabled = busy || (stopped && !state.canStart);
    captureLabel.textContent = stopped ? t("start") : t("stop");
    capture.title = stopped && !state.canStart ? t("captureHint") : "";
    lock.disabled = busy || !state.canLock;
    lock.title = state.canLock ? "Bloquear janela" : "O bloqueio precisa de atalho ou bandeja disponíveis";
    if (document.activeElement !== shortcut) shortcut.value = state.shortcut;
    updateSettingsAvailability();
    const shortcutStatus = get("#shortcut-status");
    shortcutStatus.textContent = state.shortcutAvailable
      ? "" : t("shortcutUnavailableTray");
    shortcutStatus.hidden = state.shortcutAvailable;
    if (state.error) showError(errorMessage({ code: state.error }));
    renderOutputs();
  }

  function accept(incoming: AppSnapshot) {
    if (disposed || (state && incoming.revision < state.revision)) return;
    state = incoming;
    render();
  }

  function acceptCaption(update: import("./contracts").CaptionUpdate) {
    if (!state || disposed || state.capture === "stopped") return;
    presenter.accept(update);
  }

  function showError(message: string) {
    if (disposed) return;
    error.textContent = message;
    error.hidden = false;
  }

  async function perform(action: () => Promise<AppSnapshot>) {
    if (busy || !state || disposed) return false;
    busy = true;
    error.hidden = true;
    render();
    try { accept(await action()); return true; }
    catch (failure) {
      // A state event may fail after Rust commits the operation; re-query before retrying.
      try { accept(await backend.getState()); } catch { /* Keep last confirmed state. */ }
      showError(errorMessage(failure));
      return false;
    } finally { busy = false; render(); }
  }

  // As configurações abrem por cima da área de legenda, com rolagem própria;
  // a janela nunca é redimensionada ou movida pelo app (decisão de produto).
  function toggleSettings(open: boolean) {
    if (open && state) setAppearanceInputs(state.appearance);
    settings.hidden = !open;
    preview.hidden = open;
    overlay.dataset.settings = open ? "open" : "closed";
    settingsToggle.setAttribute("aria-expanded", String(open));
    if (open) {
      void loadOutputs();
    } else {
      appearanceDraft = undefined;
    }
  }

  const options = { signal: listeners.signal };
  capture.addEventListener("click", () => void perform(() => state?.capture === "stopped" ? backend.start() : backend.stop()), options);
  language.addEventListener("click", () => toggleLanguageMenu(Boolean(languageMenu.hidden)), options);
  languageMenu.addEventListener("click", (event) => {
    const item = (event.target as HTMLElement).closest<HTMLLIElement>("li[data-value]");
    if (!item || item.getAttribute("aria-disabled") === "true") return;
    toggleLanguageMenu(false);
    const mode = item.dataset.value as CaptionMode;
    if (mode === state?.mode) return;
    void perform(() => backend.setMode(mode));
  }, options);
  uiLanguage.addEventListener("click", () => toggleUiLanguageMenu(Boolean(uiLanguageMenu.hidden)), options);
  uiLanguageMenu.addEventListener("click", (event) => {
    const item = (event.target as HTMLElement).closest<HTMLLIElement>("li[data-value]");
    if (!item) return;
    toggleUiLanguageMenu(false);
    const tag = item.dataset.value!;
    if (tag === localeTag()) return;
    void perform(() => backend.setUiLanguage(tag));
  }, options);
  document.addEventListener("pointerdown", (event) => {
    const insidePicker = (event.target as HTMLElement).closest(".language-picker");
    if (!languageMenu.hidden && !insidePicker) toggleLanguageMenu(false);
    if (!uiLanguageMenu.hidden && !insidePicker) toggleUiLanguageMenu(false);
  }, options);
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (!languageMenu.hidden) toggleLanguageMenu(false);
    if (!uiLanguageMenu.hidden) toggleUiLanguageMenu(false);
  }, options);
  lock.addEventListener("click", () => void perform(() => backend.setWindowMode("locked")), options);
  saveSettings.addEventListener("click", () => {
    if (!state || !appearanceDraft || !appearanceValid) return;
    const requestedAppearance = { ...appearanceDraft };
    const requestedShortcut = shortcut.value;
    void perform(async () => {
      let saved = state!;
      if (!sameAppearance(requestedAppearance, saved.appearance)) {
        saved = await backend.setAppearance(requestedAppearance);
      }
      if (requestedShortcut !== saved.shortcut) {
        saved = await backend.setShortcut(requestedShortcut);
      }
      return saved;
    })
      .then((saved) => {
        if (saved && !disposed) { toggleSettings(false); settingsToggle.focus(); }
      });
  }, options);
  audioOutput.addEventListener("change", () => {
    if (busy || outputBusy || !audioOutput.value) return;
    const id = audioOutput.value;
    outputBusy = true;
    audioOutput.disabled = true;
    let reload = false;
    void backend.selectAudioOutput(id).then((selection) => { outputs = selection; })
      .catch((error) => { showError(errorMessage(error)); reload = true; })
      .finally(() => { outputBusy = false; renderOutputs(); if (reload) void loadOutputs(); });
  }, options);
  settingsToggle.addEventListener("click", () => {
    const open = settings.hidden !== false;
    toggleSettings(open);
  }, options);
  root.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !settings.hidden) { toggleSettings(false); settingsToggle.focus(); }
  }, options);
  fontSize.addEventListener("input", () => {
    if (!appearanceDraft) return;
    appearanceDraft.fontSize = Number(fontSize.value);
    renderAppearancePreview();
  }, options);
  fontColor.addEventListener("input", () => {
    fontColorValue.value = fontColor.value.toUpperCase();
    updateDraftColor(fontColor.value, fontColorValue, fontColor, "color");
  }, options);
  partialColor.addEventListener("input", () => {
    partialColorValue.value = partialColor.value.toUpperCase();
    updateDraftColor(partialColor.value, partialColorValue, partialColor, "partialColor");
  }, options);
  fontColorValue.addEventListener("input", () => {
    updateDraftColor(fontColorValue.value, fontColorValue, fontColor, "color");
  }, options);
  partialColorValue.addEventListener("input", () => {
    updateDraftColor(partialColorValue.value, partialColorValue, partialColor, "partialColor");
  }, options);
  for (const input of [fontColorValue, partialColorValue]) {
    input.addEventListener("blur", () => {
      const normalized = normalizeCaptionColor(input.value);
      if (normalized) input.value = normalized;
    }, options);
  }
  resetAppearance.addEventListener("click", () => setAppearanceInputs(DEFAULT_APPEARANCE), options);
  window.addEventListener("resize", () => presenter.refreshLayout(), options);
  for (const [selector, direction] of [
    ["#resize-left", "West"],
    ["#resize-right", "East"],
    ["#resize-bottom", "South"],
  ] as const satisfies readonly [string, ResizeDirection][]) {
    get(selector).addEventListener("mousedown", (event) => {
      if (event.button !== 0) return;
      event.preventDefault();
      void backend.resize(direction).catch(() => showError(t("errResize")));
    }, options);
  }
  // Fecha direto para a bandeja, sem aviso (decisão de produto).
  get("#close").addEventListener("click", () => {
    void backend.close().catch(() => showError(t("errTrayClose")));
  }, options);

  renderUiLanguageButton();

  const ready = (async () => {
    try {
      const release = await backend.subscribe(accept);
      if (disposed) { release(); return; }
      unsubscribe = release;
      const releaseCaptions = await backend.subscribeCaptions(acceptCaption);
      if (disposed) { releaseCaptions(); return; }
      unsubscribeCaptions = releaseCaptions;
      accept(await backend.getState());
    } catch {
      if (!disposed) {
        showError(t("errConnect"));
      }
    }
  })();

  return { ready, dispose() {
    if (disposed) return;
    disposed = true;
    listeners.abort();
    unsubscribe?.();
    unsubscribeCaptions?.();
    presenter.dispose();
  } };
}
