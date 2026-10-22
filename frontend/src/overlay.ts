import type { AppSnapshot, Appearance, Backend, CaptionColor, CaptionMode, OutputSelection } from "./contracts";
import { CaptionPresenter } from "./captions";

const icons = {
  play: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 5 11 7-11 7Z"/></svg>',
  lock: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0"/></svg>',
  settings: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/></svg>',
  close: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>',
};

function errorMessage(error: unknown): string {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : null;
  switch (code) {
    case "capture-unavailable": return "A captura de áudio ainda está indisponível.";
    case "transcription-unavailable": return "O modelo de transcrição está indisponível.";
    case "translation-unavailable": return "O modelo de tradução está indisponível.";
    case "invalid-font-size": return "Escolha um tamanho de texto entre 20 e 56 px.";
    case "recovery-unavailable": return "O bloqueio precisa de um atalho ou da bandeja para recuperar os controles.";
    case "invalid-shortcut": return "Use um atalho no formato Ctrl+Alt+L (letra de A a Z).";
    case "shortcut-unavailable": return "Esse atalho está em uso. Escolha outra letra; a bandeja continua disponível.";
    case "not-found": return "Essa saída de áudio não está disponível. Atualize a lista e escolha outra.";
    case "invalid-id": return "Selecione uma saída de áudio válida.";
    default: return "Não foi possível concluir a ação. Tente novamente.";
  }
}

export function createOverlay(root: HTMLElement, backend: Backend) {
  // Only this trusted, static template uses HTML. All variable text uses textContent.
  root.innerHTML = `
    <section class="overlay" data-mode="edit" aria-label="Janela de legendas">
      <header class="toolbar">
        <div class="brand" data-tauri-drag-region title="Arraste para mover a janela">
          <span class="brand-mark" data-tauri-drag-region aria-hidden="true">cc</span>
          <span class="brand-name" data-tauri-drag-region>AnyCaptions</span>
        </div>
        <div class="drag-region" data-tauri-drag-region title="Arraste para mover a janela"></div>
        <div class="controls" aria-label="Controles de legendas">
          <button id="capture" class="capture-button" disabled>${icons.play}<span id="capture-label">Iniciar</span></button>
          <label class="sr-only" for="language">Idioma das legendas</label>
          <select id="language" disabled><option value="en-en" title="Detecta o idioma e transcreve no idioma falado">Auto</option><option value="en-pt">EN → PT</option></select>
          <button id="settings-toggle" class="icon-button" aria-label="Configurações" title="Configurações" aria-expanded="false" aria-controls="settings">${icons.settings}</button>
          <button id="lock" class="icon-button" aria-label="Bloquear janela" title="O bloqueio precisa de atalho ou bandeja disponíveis" disabled>${icons.lock}</button>
        </div>
          <button id="close" class="icon-button close-button" aria-label="Recolher à bandeja" title="Recolher à bandeja">${icons.close}</button>
      </header>
      <div class="content">
        <div id="preview" class="preview">
          <p id="caption-preview" class="caption" data-color="white">Suas legendas aparecem aqui</p>
          <p id="caption-live" class="caption" data-color="white" aria-live="polite" hidden></p>
        </div>
        <section id="settings" class="settings" aria-label="Configurações de legendas" hidden>
          <h2>Configurações</h2>
          <div class="setting-row">
            <label for="font-size">Tamanho <output id="font-value" for="font-size">32 px</output></label>
            <input id="font-size" type="range" min="20" max="56" value="32" step="1" disabled />
          </div>
          <div class="setting-row">
            <label for="font-color">Cor do texto</label>
            <select id="font-color" disabled><option value="white">Branco</option><option value="yellow">Amarelo</option></select>
          </div>
          <div class="setting-row">
            <label for="audio-output">Saída de áudio</label>
            <select id="audio-output" disabled><option>Nenhuma saída disponível</option></select>
          </div>
          <p id="audio-output-status" class="muted" hidden></p>
          <div class="setting-row">
            <label for="shortcut">Atalho de bloqueio</label>
            <div class="setting-actions">
              <input id="shortcut" type="text" value="Ctrl+Alt+L" aria-describedby="shortcut-status" maxlength="16" spellcheck="false" />
            </div>
          </div>
          <p id="shortcut-status" class="muted" hidden></p>
          <div class="settings-footer"><button id="save-settings" type="button">Salvar</button></div>
        </section>
      </div>
      <p id="notice" class="notice" role="status" hidden></p>
      <p id="error" class="error" role="alert" hidden></p>
      <div id="resize" class="resize-grip" title="Arraste para redimensionar" aria-hidden="true"></div>
    </section>`;

  function get<T extends HTMLElement>(selector: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`Missing overlay element: ${selector}`);
    return element;
  }

  const overlay = get<HTMLElement>(".overlay");
  const capture = get<HTMLButtonElement>("#capture");
  const captureLabel = get<HTMLElement>("#capture-label");
  const language = get<HTMLSelectElement>("#language");
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
  const fontColor = get<HTMLSelectElement>("#font-color");
  const shortcut = get<HTMLInputElement>("#shortcut");
  const audioOutput = get<HTMLSelectElement>("#audio-output");
  const error = get<HTMLElement>("#error");
  const listeners = new AbortController();
  let state: AppSnapshot | undefined;
  let busy = false;
  let disposed = false;
  let unsubscribe: (() => void) | undefined;
  let unsubscribeCaptions: (() => void) | undefined;
  let closeTimer: ReturnType<typeof setTimeout> | undefined;
  let hasExplainedClose = false;
  let previousCapture: AppSnapshot["capture"] | undefined;
  let outputs: OutputSelection | undefined;
  let outputBusy = false;

  function renderOutputs() {
    audioOutput.replaceChildren();
    if (!outputs) {
      audioOutput.add(new Option("Verificando saídas…", ""));
      audioOutput.disabled = true;
      return;
    }
    for (const device of outputs.devices) audioOutput.add(new Option(device.name, device.id));
    if (outputs.devices.length && !outputs.selectedId) audioOutput.add(new Option("Selecione uma saída", ""), 0);
    if (outputs.selectedId && !outputs.selectedAvailable) {
      audioOutput.add(new Option("Saída salva indisponível", outputs.selectedId));
    }
    if (!outputs.devices.length) audioOutput.add(new Option("Nenhuma saída disponível", ""));
    audioOutput.value = outputs.selectedId ?? "";
    audioOutput.disabled = busy || outputBusy || !outputs.devices.length;
    const outputStatus = get("#audio-output-status");
    outputStatus.textContent = outputs.selectedId && !outputs.selectedAvailable
      ? "A saída salva foi desconectada. Escolha outra saída."
      : outputs.devices.length ? "" : "Nenhuma saída de reprodução ativa.";
    outputStatus.hidden = !outputStatus.textContent;
    get<HTMLButtonElement>("#save-settings").disabled = busy || outputBusy;
  }

  async function loadOutputs() {
    if (outputBusy || disposed) return;
    outputBusy = true;
    renderOutputs();
    try { outputs = await backend.getAudioOutputs(); }
    catch { showError("Não foi possível listar as saídas de áudio."); }
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
    language.value = state.mode;
    language.disabled = busy;
    fontSize.value = String(state.appearance.fontSize);
    fontColor.value = state.appearance.color;
    fontSize.disabled = fontColor.disabled = busy;
    caption.style.setProperty("--caption-base-size", `${state.appearance.fontSize}px`);
    live.style.setProperty("--caption-base-size", `${state.appearance.fontSize}px`);
    caption.dataset.color = state.appearance.color;
    live.dataset.color = state.appearance.color;
    get("#font-value").textContent = `${state.appearance.fontSize} px`;
    const stopped = state.capture === "stopped";
    capture.disabled = busy || (stopped && !state.canStart);
    captureLabel.textContent = stopped ? "Iniciar" : "Parar";
    capture.title = stopped && !state.canStart ? "A captura precisa de uma saída de áudio e de modelos disponíveis" : "";
    lock.disabled = busy || !state.canLock;
    lock.title = state.canLock ? "Bloquear janela" : "O bloqueio precisa de atalho ou bandeja disponíveis";
    if (document.activeElement !== shortcut) shortcut.value = state.shortcut;
    shortcut.disabled = get<HTMLButtonElement>("#save-settings").disabled = busy;
    const shortcutStatus = get("#shortcut-status");
    shortcutStatus.textContent = state.shortcutAvailable
      ? "" : "Atalho indisponível; use a bandeja para recuperar os controles.";
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

  function toggleSettings(open: boolean) {
    settings.hidden = !open;
    preview.hidden = open;
    overlay.dataset.settings = open ? "open" : "closed";
    settingsToggle.setAttribute("aria-expanded", String(open));
    if (open) {
      void backend.fitSettings().catch(() => showError("Não foi possível ampliar as configurações."));
      void loadOutputs();
    }
  }

  const options = { signal: listeners.signal };
  capture.addEventListener("click", () => void perform(() => state?.capture === "stopped" ? backend.start() : backend.stop()), options);
  language.addEventListener("change", () => {
    const mode = language.value as CaptionMode;
    void perform(() => backend.setMode(mode));
  }, options);
  lock.addEventListener("click", () => void perform(() => backend.setWindowMode("locked")), options);
  get("#save-settings").addEventListener("click", () => {
    const requested = shortcut.value;
    void perform(() => requested === state?.shortcut ? backend.getState() : backend.setShortcut(requested))
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
  function saveAppearance() {
    const appearance: Appearance = { fontSize: Number(fontSize.value), color: fontColor.value as CaptionColor };
    void perform(() => backend.setAppearance(appearance));
  }
  fontSize.addEventListener("change", saveAppearance, options);
  fontColor.addEventListener("change", saveAppearance, options);
  get("#resize").addEventListener("mousedown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    void backend.resize().catch(() => showError("Não foi possível redimensionar a janela."));
  }, options);
  get("#close").addEventListener("click", () => {
    if (!hasExplainedClose) {
      hasExplainedClose = true;
      const notice = get<HTMLElement>("#notice");
      notice.textContent = "A janela será recolhida. Use a bandeja para mostrar e editar ou sair.";
      notice.hidden = false;
      closeTimer = setTimeout(() => { void backend.close().catch(() => showError("Não foi possível recolher a janela.")); }, 1800);
    } else {
      if (closeTimer !== undefined) clearTimeout(closeTimer);
      void backend.close().catch(() => showError("Não foi possível recolher a janela."));
    }
  }, options);

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
        showError("Não foi possível conectar à aplicação. Feche e abra novamente.");
      }
    }
  })();

  return { ready, dispose() {
    if (disposed) return;
    disposed = true;
    listeners.abort();
    if (closeTimer !== undefined) clearTimeout(closeTimer);
    unsubscribe?.();
    unsubscribeCaptions?.();
    presenter.dispose();
  } };
}
