import {
  ASR_LANGUAGES,
  TRANSLATION_ENGINES,
  TRANSLATION_TARGETS,
  engineServes,
  flagUrl,
} from "./languages";
import { UI_LANGUAGES, localeTag, setLocale, t, type MessageKey } from "./i18n";
import type {
  AppSnapshot,
  Appearance,
  Backend,
  CaptionMode,
  ResizeDirection,
  OutputSelection,
  TranslationEngine,
} from "./contracts";
import {
  DEFAULT_APPEARANCE,
  applyAppearance,
  normalizeCaptionColor,
  sameAppearance,
} from "./appearance";
import { CaptionPresenter } from "./captions";
import { DictationIndicator } from "./dictation";
import type { DictationActivation, DictationSettings, DictationSnapshot } from "./contracts";

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
    default: break;
  }
  const kind = typeof error === "object" && error !== null && "kind" in error ? error.kind : null;
  if (kind === "dictation") {
    switch (code) {
      case "invalid-shortcut": return t("errDictationShortcutFormat");
      case "shortcut-conflict": return t("errDictationShortcutConflict");
      case "invalid-duration": return t("errDictationDuration");
      case "invalid-language": return t("errDictationLanguage");
      default: return t("errDictation");
    }
  }
  return t("errGeneric");
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
            <label for="source-language" data-i18n="sourceLanguage">Idioma falado (origem)</label>
            <div class="language-picker">
              <button id="source-language" type="button" aria-haspopup="listbox" aria-expanded="false"><span id="source-language-label"></span></button>
              <ul id="source-language-menu" role="listbox" data-i18n-aria="sourceLanguage" aria-label="Idioma falado (origem)" hidden></ul>
            </div>
          </div>
          <div class="setting-row">
            <label for="translation-engine" data-i18n="translationEngine">Modelo de tradução</label>
            <select id="translation-engine"></select>
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
          <h3 class="settings-subtitle" data-i18n="dictation">Ditado por voz</h3>
          <div class="setting-row">
            <label for="dictation-enabled" data-i18n="dictationEnable">Ativar ditado</label>
            <input id="dictation-enabled" type="checkbox" />
          </div>
          <div class="setting-row">
            <label for="dictation-mic" data-i18n="dictationMic">Microfone</label>
            <select id="dictation-mic"></select>
          </div>
          <div class="setting-row">
            <label for="dictation-shortcut" data-i18n="dictationShortcut">Atalho do ditado</label>
            <div class="setting-actions">
              <input id="dictation-shortcut" type="text" value="Ctrl+Space" maxlength="24" spellcheck="false" />
            </div>
          </div>
          <div class="setting-row">
            <label for="dictation-language" data-i18n="dictationLanguage">Idioma do ditado</label>
            <div class="language-picker">
              <button id="dictation-language" type="button" aria-haspopup="listbox" aria-expanded="false"><span id="dictation-language-label"></span></button>
              <ul id="dictation-language-menu" role="listbox" data-i18n-aria="dictationLanguage" aria-label="Idioma do ditado" hidden></ul>
            </div>
          </div>
          <div class="setting-row">
            <label for="dictation-translate" data-i18n="dictationTranslate">Traduzir ditado para</label>
            <div class="language-picker">
              <button id="dictation-translate" type="button" aria-haspopup="listbox" aria-expanded="false"><span id="dictation-translate-label"></span></button>
              <ul id="dictation-translate-menu" role="listbox" data-i18n-aria="dictationTranslate" aria-label="Traduzir ditado para" hidden></ul>
            </div>
          </div>
          <div class="setting-row">
            <label for="dictation-activation" data-i18n="dictationActivation">Modo de ativação</label>
            <select id="dictation-activation">
              <option value="hold" data-i18n="dictationHold">Segurar para falar</option>
              <option value="toggle" data-i18n="dictationToggle">Alternar (toque liga e desliga)</option>
            </select>
          </div>
          <div class="setting-row">
            <label for="dictation-sound" data-i18n="dictationSound">Sinal sonoro ao gravar</label>
            <input id="dictation-sound" type="checkbox" />
          </div>
          <div class="setting-row">
            <label for="dictation-show-text" data-i18n="dictationShowText">Mostrar texto reconhecido</label>
            <input id="dictation-show-text" type="checkbox" />
          </div>
          <div class="setting-row">
            <label for="dictation-limit" data-i18n="dictationLimitDuration">Limitar duração</label>
            <input id="dictation-limit" type="checkbox" />
          </div>
          <div class="setting-row">
            <label for="dictation-duration" data-i18n="dictationMaxDuration">Duração máxima (s)</label>
            <input id="dictation-duration" type="number" min="5" max="300" step="5" value="60" />
          </div>
          <p id="dictation-status" class="muted" hidden></p>
          <div class="settings-footer">
            <button id="developer-credit" class="settings-credit" type="button">developed by EduardoViniciusJ</button>
            <button id="reset-appearance" class="secondary-button" type="button" data-i18n="restoreDefault">Restaurar padrão</button>
            <button id="save-settings" type="button" data-i18n="save">Salvar</button>
          </div>
        </section>
      </div>
      <div id="dictation-indicator" class="dictation-indicator" hidden>
        <span class="dictation-dot" aria-hidden="true"></span>
        <span id="dictation-text" class="dictation-text" role="status"></span>
        <span id="dictation-elapsed" class="dictation-elapsed"></span>
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
      // jsdom (testes) não implementa scrollIntoView.
      selected?.scrollIntoView?.({ block: "nearest" });
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

  // Seletor do idioma do ditado (configurações): os 83 idiomas do dicionário
  // de prompts do modelo, com as mesmas bandeiras dos demais seletores.
  // "" = detecção automática (ícone de globo).
  const dictationLanguage = get<HTMLButtonElement>("#dictation-language");
  const dictationLanguageLabel = get<HTMLElement>("#dictation-language-label");
  const dictationLanguageMenu = get<HTMLUListElement>("#dictation-language-menu");
  let dictationLanguageValue = "";
  {
    const auto = document.createElement("li");
    auto.role = "option";
    auto.dataset.value = "";
    const globe = document.createElement("span");
    globe.className = "flag flag-globe";
    globe.innerHTML = icons.globe;
    auto.append(globe);
    const text = document.createElement("span");
    text.textContent = t("autoMode");
    auto.append(text);
    dictationLanguageMenu.append(auto);
  }
  for (const option of ASR_LANGUAGES) {
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
    dictationLanguageMenu.append(item);
  }
  const dictationLanguageItems = [
    ...dictationLanguageMenu.querySelectorAll<HTMLLIElement>("li"),
  ];
  const toggleDictationLanguageMenu = (open: boolean) => {
    dictationLanguageMenu.hidden = !open;
    dictationLanguage.setAttribute("aria-expanded", String(open));
    if (open) {
      const selected = dictationLanguageItems.find(
        (item) => item.dataset.value === dictationLanguageValue,
      );
      // jsdom (testes) não implementa scrollIntoView.
      selected?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const renderDictationLanguageButton = () => {
    const item =
      dictationLanguageItems.find((entry) => entry.dataset.value === dictationLanguageValue)
      ?? dictationLanguageItems[0];
    if (!item) return;
    dictationLanguageLabel.textContent = item.querySelector("span:not(.flag)")?.textContent ?? "";
    dictationLanguage.querySelector(".flag")?.remove();
    const flag = item.querySelector(".flag");
    if (flag) dictationLanguage.prepend(flag.cloneNode(true));
    for (const entry of dictationLanguageItems) {
      entry.setAttribute("aria-selected", String(entry === item));
    }
  };
  renderDictationLanguageButton();

  // Seletor do alvo de tradução do ditado: "Não traduzir" + os alvos do
  // mecanismo de tradução escolhido (o render filtra, como no menu das
  // legendas). Valor em rascunho, confirmado pelo botão Salvar.
  const dictationTranslate = get<HTMLButtonElement>("#dictation-translate");
  const dictationTranslateLabel = get<HTMLElement>("#dictation-translate-label");
  const dictationTranslateMenu = get<HTMLUListElement>("#dictation-translate-menu");
  let dictationTranslateValue = "";
  {
    const none = document.createElement("li");
    none.role = "option";
    none.dataset.value = "";
    const text = document.createElement("span");
    text.textContent = t("dictationNoTranslate");
    none.append(text);
    dictationTranslateMenu.append(none);
  }
  for (const target of TRANSLATION_TARGETS) {
    const item = document.createElement("li");
    item.role = "option";
    item.dataset.value = target.mode.replace("translate-", "");
    item.title = target.name;
    const flag = flagUrl(target.country);
    if (flag) {
      const image = document.createElement("img");
      image.className = "flag";
      image.src = flag;
      image.alt = "";
      item.append(image);
    }
    const text = document.createElement("span");
    text.textContent = target.name;
    item.append(text);
    dictationTranslateMenu.append(item);
  }
  const dictationTranslateItems = [
    ...dictationTranslateMenu.querySelectorAll<HTMLLIElement>("li"),
  ];
  const toggleDictationTranslateMenu = (open: boolean) => {
    dictationTranslateMenu.hidden = !open;
    dictationTranslate.setAttribute("aria-expanded", String(open));
    if (open) {
      const selected = dictationTranslateItems.find(
        (item) => item.dataset.value === dictationTranslateValue,
      );
      // jsdom (testes) não implementa scrollIntoView.
      selected?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const renderDictationTranslateButton = () => {
    // Alvos fora do mecanismo atual saem do menu; um valor salvo que ficou
    // fora volta a "Não traduzir" na exibição.
    const engine = state?.translationEngine ?? "opus";
    for (const entry of dictationTranslateItems) {
      const tag = entry.dataset.value ?? "";
      entry.hidden = tag !== "" && !engineServes(engine, `translate-${tag}` as CaptionMode);
    }
    const item =
      dictationTranslateItems.find(
        (entry) => entry.dataset.value === dictationTranslateValue && !entry.hidden,
      ) ?? dictationTranslateItems[0];
    if (!item) return;
    dictationTranslateValue = item.dataset.value ?? "";
    dictationTranslateLabel.textContent =
      item.querySelector("span:not(.flag)")?.textContent ?? "";
    dictationTranslate.querySelector(".flag")?.remove();
    const flag = item.querySelector(".flag");
    if (flag) dictationTranslate.prepend(flag.cloneNode(true));
    for (const entry of dictationTranslateItems) {
      entry.setAttribute("aria-selected", String(entry === item));
    }
  };

  // Seletor do idioma falado (origem) das legendas: mesma lista do modelo
  // ASR; fixar condiciona o reconhecedor e dá a origem exata à tradução.
  // Diferente do ditado, aplica na hora (via Rust), como o mecanismo.
  const sourceLanguage = get<HTMLButtonElement>("#source-language");
  const sourceLanguageLabel = get<HTMLElement>("#source-language-label");
  const sourceLanguageMenu = get<HTMLUListElement>("#source-language-menu");
  {
    const auto = document.createElement("li");
    auto.role = "option";
    auto.dataset.value = "";
    const globe = document.createElement("span");
    globe.className = "flag flag-globe";
    globe.innerHTML = icons.globe;
    auto.append(globe);
    const text = document.createElement("span");
    text.textContent = t("autoMode");
    auto.append(text);
    sourceLanguageMenu.append(auto);
  }
  for (const option of ASR_LANGUAGES) {
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
    sourceLanguageMenu.append(item);
  }
  const sourceLanguageItems = [...sourceLanguageMenu.querySelectorAll<HTMLLIElement>("li")];
  // Valor confirmado pelo backend (snapshot); "" = detecção automática.
  let sourceLanguageValue = "";
  const toggleSourceLanguageMenu = (open: boolean) => {
    sourceLanguageMenu.hidden = !open;
    sourceLanguage.setAttribute("aria-expanded", String(open));
    if (open) {
      const selected = sourceLanguageItems.find(
        (item) => item.dataset.value === sourceLanguageValue,
      );
      // jsdom (testes) não implementa scrollIntoView.
      selected?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const renderSourceLanguageButton = () => {
    const item =
      sourceLanguageItems.find((entry) => entry.dataset.value === sourceLanguageValue)
      ?? sourceLanguageItems[0];
    if (!item) return;
    sourceLanguageLabel.textContent = item.querySelector("span:not(.flag)")?.textContent ?? "";
    sourceLanguage.querySelector(".flag")?.remove();
    const flag = item.querySelector(".flag");
    if (flag) sourceLanguage.prepend(flag.cloneNode(true));
    for (const entry of sourceLanguageItems) {
      entry.setAttribute("aria-selected", String(entry === item));
    }
  };
  renderSourceLanguageButton();

  // Seletor do mecanismo de tradução (configurações); o menu de idiomas
  // mostra apenas os alvos que o mecanismo escolhido serve.
  const translationEngine = get<HTMLSelectElement>("#translation-engine");
  for (const engine of TRANSLATION_ENGINES) {
    translationEngine.add(new Option(t(engine.labelKey), engine.id));
  }
  const renderEngineOptions = () => {
    for (const option of translationEngine.options) {
      const engine = TRANSLATION_ENGINES.find((entry) => entry.id === option.value);
      if (engine) option.textContent = t(engine.labelKey);
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
    const dictationAuto = dictationLanguageItems[0]?.querySelector("span:not(.flag)");
    if (dictationAuto) dictationAuto.textContent = t("autoMode");
    const dictationNone = dictationTranslateItems[0]?.querySelector("span:not(.flag)");
    if (dictationNone) dictationNone.textContent = t("dictationNoTranslate");
    const sourceAuto = sourceLanguageItems[0]?.querySelector("span:not(.flag)");
    if (sourceAuto) sourceAuto.textContent = t("autoMode");
    renderSourceLanguageButton();
    renderEngineOptions();
    renderUiLanguageButton();
    renderDictation();
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
  const dictationEnabled = get<HTMLInputElement>("#dictation-enabled");
  const dictationMic = get<HTMLSelectElement>("#dictation-mic");
  const dictationShortcut = get<HTMLInputElement>("#dictation-shortcut");
  const dictationActivation = get<HTMLSelectElement>("#dictation-activation");
  const dictationSound = get<HTMLInputElement>("#dictation-sound");
  const dictationShowText = get<HTMLInputElement>("#dictation-show-text");
  const dictationLimit = get<HTMLInputElement>("#dictation-limit");
  const dictationDuration = get<HTMLInputElement>("#dictation-duration");
  const dictationStatus = get<HTMLElement>("#dictation-status");
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
  let dictation: DictationSnapshot | undefined;
  let dictationBusy = false;
  let unsubscribeDictation: (() => void) | undefined;
  const dictationIndicator = new DictationIndicator(
    get("#dictation-indicator"),
    get("#dictation-text"),
    get("#dictation-elapsed"),
    showError,
  );

  function renderDictation() {
    if (!dictation) return;
    const { settings, inputs, available } = dictation;
    dictationEnabled.checked = settings.enabled;
    if (document.activeElement !== dictationShortcut) dictationShortcut.value = settings.shortcut;
    dictationActivation.value = settings.activation;
    dictationSound.checked = settings.soundCue;
    dictationLanguageValue = settings.language ?? "";
    // Idioma salvo que saiu da lista volta à detecção automática na exibição.
    if (!dictationLanguageItems.some((item) => item.dataset.value === dictationLanguageValue)) {
      dictationLanguageValue = "";
    }
    renderDictationLanguageButton();
    dictationTranslateValue = settings.translateTo ?? "";
    renderDictationTranslateButton();
    dictationShowText.checked = settings.showLiveText;
    dictationLimit.checked = settings.limitDuration;
    dictationDuration.value = String(settings.maxDurationSecs);
    dictationDuration.disabled = !settings.limitDuration;
    dictationMic.replaceChildren();
    dictationMic.add(new Option(t("dictationDefaultMic"), ""));
    for (const device of inputs.devices) dictationMic.add(new Option(device.name, device.id));
    dictationMic.value = settings.inputId ?? "";
    // Microfone salvo que desapareceu volta ao padrão do sistema na exibição.
    if (dictationMic.value !== (settings.inputId ?? "")) dictationMic.value = "";
    dictationStatus.textContent = available ? "" : t("dictationModelMissing");
    dictationStatus.hidden = available;
  }

  async function loadDictation() {
    if (dictationBusy || disposed) return;
    dictationBusy = true;
    try {
      dictation = await backend.getDictation();
      renderDictation();
    } catch {
      /* A seção mantém os últimos valores conhecidos. */
    } finally {
      dictationBusy = false;
    }
  }

  function dictationDraft(): DictationSettings | undefined {
    if (!dictation) return undefined;
    return {
      enabled: dictationEnabled.checked,
      shortcut: dictationShortcut.value.trim(),
      activation: dictationActivation.value as DictationActivation,
      soundCue: dictationSound.checked,
      showLiveText: dictationShowText.checked,
      limitDuration: dictationLimit.checked,
      maxDurationSecs: Number(dictationDuration.value),
      inputId: dictationMic.value || null,
      language: dictationLanguageValue || null,
      translateTo: dictationTranslateValue || null,
    };
  }

  function sameDictation(a: DictationSettings, b: DictationSettings): boolean {
    return a.enabled === b.enabled
      && a.shortcut === b.shortcut
      && a.activation === b.activation
      && a.soundCue === b.soundCue
      && a.showLiveText === b.showLiveText
      && a.limitDuration === b.limitDuration
      && a.maxDurationSecs === b.maxDurationSecs
      && (a.inputId ?? "") === (b.inputId ?? "")
      && (a.language ?? "") === (b.language ?? "")
      && (a.translateTo ?? "") === (b.translateTo ?? "");
  }

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
    translationEngine.disabled = disabled;
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
    sourceLanguageValue = state.sourceLanguage ?? "";
    renderSourceLanguageButton();
    sourceLanguage.disabled = busy;
    // Troca de mecanismo refiltra os alvos de tradução do ditado.
    renderDictationTranslateButton();
    if (busy || state.windowMode === "locked") {
      toggleLanguageMenu(false);
      toggleUiLanguageMenu(false);
      toggleSourceLanguageMenu(false);
    }
    translationEngine.value = state.translationEngine;
    for (const item of languageItems) {
      const translate = item.dataset.value?.startsWith("translate-") ?? false;
      // Alvos fora do mecanismo escolhido saem do menu; os demais só
      // desabilitam enquanto o modelo não está instalado.
      item.hidden =
        translate && !engineServes(state.translationEngine, item.dataset.value as CaptionMode);
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
      void loadDictation();
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
  translationEngine.addEventListener("change", () => {
    if (!state || busy) return;
    const engine = translationEngine.value as TranslationEngine;
    if (engine === state.translationEngine) return;
    void perform(() => backend.setTranslationEngine(engine));
  }, options);
  sourceLanguage.addEventListener(
    "click",
    () => toggleSourceLanguageMenu(Boolean(sourceLanguageMenu.hidden)),
    options,
  );
  sourceLanguageMenu.addEventListener("click", (event) => {
    const item = (event.target as HTMLElement).closest<HTMLLIElement>("li[data-value]");
    if (!item || !state || busy) return;
    toggleSourceLanguageMenu(false);
    const tag = item.dataset.value ?? "";
    if (tag === sourceLanguageValue) return;
    void perform(() => backend.setSourceLanguage(tag || null));
  }, options);
  dictationTranslate.addEventListener(
    "click",
    () => toggleDictationTranslateMenu(Boolean(dictationTranslateMenu.hidden)),
    options,
  );
  dictationTranslateMenu.addEventListener("click", (event) => {
    const item = (event.target as HTMLElement).closest<HTMLLIElement>("li[data-value]");
    if (!item) return;
    toggleDictationTranslateMenu(false);
    dictationTranslateValue = item.dataset.value ?? "";
    renderDictationTranslateButton();
  }, options);
  dictationLanguage.addEventListener(
    "click",
    () => toggleDictationLanguageMenu(Boolean(dictationLanguageMenu.hidden)),
    options,
  );
  dictationLanguageMenu.addEventListener("click", (event) => {
    const item = (event.target as HTMLElement).closest<HTMLLIElement>("li[data-value]");
    if (!item) return;
    toggleDictationLanguageMenu(false);
    dictationLanguageValue = item.dataset.value ?? "";
    renderDictationLanguageButton();
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
    if (!dictationLanguageMenu.hidden && !insidePicker) toggleDictationLanguageMenu(false);
    if (!dictationTranslateMenu.hidden && !insidePicker) toggleDictationTranslateMenu(false);
    if (!sourceLanguageMenu.hidden && !insidePicker) toggleSourceLanguageMenu(false);
  }, options);
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (!languageMenu.hidden) toggleLanguageMenu(false);
    if (!uiLanguageMenu.hidden) toggleUiLanguageMenu(false);
    if (!dictationLanguageMenu.hidden) toggleDictationLanguageMenu(false);
    if (!dictationTranslateMenu.hidden) toggleDictationTranslateMenu(false);
    if (!sourceLanguageMenu.hidden) toggleSourceLanguageMenu(false);
  }, options);
  lock.addEventListener("click", () => void perform(() => backend.setWindowMode("locked")), options);
  saveSettings.addEventListener("click", () => {
    if (!state || !appearanceDraft || !appearanceValid) return;
    const requestedAppearance = { ...appearanceDraft };
    const requestedShortcut = shortcut.value;
    const requestedDictation = dictationDraft();
    void perform(async () => {
      let saved = state!;
      if (!sameAppearance(requestedAppearance, saved.appearance)) {
        saved = await backend.setAppearance(requestedAppearance);
      }
      if (requestedShortcut !== saved.shortcut) {
        saved = await backend.setShortcut(requestedShortcut);
      }
      if (requestedDictation && dictation && !sameDictation(requestedDictation, dictation.settings)) {
        dictation = await backend.setDictation(requestedDictation);
        renderDictation();
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
  dictationLimit.addEventListener("change", () => {
    dictationDuration.disabled = !dictationLimit.checked;
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
  get<HTMLButtonElement>("#developer-credit").addEventListener("click", () => {
    void backend.openProjectPage().catch((error) => showError(errorMessage(error)));
  }, options);
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
      const releaseDictation = await backend.subscribeDictation(
        (event) => { if (!disposed) dictationIndicator.accept(event); },
      );
      if (disposed) { releaseDictation(); return; }
      unsubscribeDictation = releaseDictation;
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
    unsubscribeDictation?.();
    dictationIndicator.dispose();
    presenter.dispose();
  } };
}
