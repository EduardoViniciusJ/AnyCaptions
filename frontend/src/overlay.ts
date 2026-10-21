import type { AppSnapshot, Appearance, Backend, CaptionColor, CaptionMode } from "./contracts";
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
          <select id="language" disabled><option value="en-en">EN → EN</option><option value="en-pt">EN → PT</option></select>
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
          <div class="settings-status">
            <p id="status" class="muted" role="status">Conectando…</p>
            <p id="availability" class="availability">Verificando disponibilidade…</p>
          </div>
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
          <div class="setting-row">
            <label for="shortcut">Atalho de bloqueio</label>
            <div class="setting-actions">
              <input id="shortcut" type="text" value="Ctrl+Alt+L" aria-describedby="shortcut-status" maxlength="16" spellcheck="false" />
              <button id="save-shortcut" type="button">Salvar</button>
            </div>
          </div>
          <p id="shortcut-status" class="muted"></p>
          <p id="models-status" class="muted"></p>
          ${import.meta.env.DEV ? '<button id="simulate" type="button">Simular legendas</button>' : ''}
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
  const presenter = new CaptionPresenter(live, undefined, () => { caption.hidden = !live.hidden; });
  const fontSize = get<HTMLInputElement>("#font-size");
  const fontColor = get<HTMLSelectElement>("#font-color");
  const shortcut = get<HTMLInputElement>("#shortcut");
  const status = get<HTMLElement>("#status");
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
  let previousSession: number | undefined;
  let simulatedSegmentId = 1;

  function render() {
    if (!state || disposed) return;
    overlay.dataset.mode = state.windowMode;
    if (previousSession !== state.sessionId) { simulatedSegmentId = 1; previousSession = state.sessionId; }
    presenter.setContext(state.sessionId, state.mode);
    if (state.capture === "stopped" && previousCapture && previousCapture !== "stopped") presenter.clear();
    previousCapture = state.capture;
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
    shortcut.value = state.shortcut;
    shortcut.disabled = get<HTMLButtonElement>("#save-shortcut").disabled = busy;
    get("#shortcut-status").textContent = state.shortcutAvailable
      ? `Atalho ativo: ${state.shortcut}` : "Atalho indisponível; use a bandeja para recuperar os controles.";
    const states: Record<AppSnapshot["capture"], string> = {
      stopped: "Captura parada", preparing: "Carregando modelos", listening: "Aguardando fala",
      processing: state.mode === "en-en" ? "Transcrevendo" : "Traduzindo", error: "Falha na captura",
    };
    status.textContent = states[state.capture];
    get("#availability").textContent = state.canStart ? "Pronto para iniciar" : "Áudio e modelos indisponíveis";
    get("#models-status").textContent = state.mode === "en-pt"
      ? "EN → PT precisa dos modelos locais de transcrição e tradução."
      : "EN → EN precisa de um modelo local de transcrição.";
  }

  function accept(incoming: AppSnapshot) {
    if (disposed || (state && incoming.revision < state.revision)) return;
    state = incoming;
    render();
  }

  function acceptCaption(update: import("./contracts").CaptionUpdate, simulated = false) {
    if (!state || disposed || (state.capture === "stopped" && !simulated)) return;
    presenter.accept(update);
  }

  function showError(message: string) {
    if (disposed) return;
    error.textContent = message;
    error.hidden = false;
  }

  async function perform(action: () => Promise<AppSnapshot>) {
    if (busy || !state || disposed) return;
    busy = true;
    error.hidden = true;
    render();
    try { accept(await action()); }
    catch (failure) {
      // A state event may fail after Rust commits the operation; re-query before retrying.
      try { accept(await backend.getState()); } catch { /* Keep last confirmed state. */ }
      showError(errorMessage(failure));
    } finally { busy = false; render(); }
  }

  function toggleSettings(open: boolean) {
    settings.hidden = !open;
    preview.hidden = open;
    settingsToggle.setAttribute("aria-expanded", String(open));
  }

  const options = { signal: listeners.signal };
  capture.addEventListener("click", () => void perform(() => state?.capture === "stopped" ? backend.start() : backend.stop()), options);
  language.addEventListener("change", () => {
    const mode = language.value as CaptionMode;
    void perform(() => backend.setMode(mode));
  }, options);
  lock.addEventListener("click", () => void perform(() => backend.setWindowMode("locked")), options);
  get("#save-shortcut").addEventListener("click", () => {
    const requested = shortcut.value;
    void perform(() => backend.setShortcut(requested));
  }, options);
  root.querySelector("#simulate")?.addEventListener("click", () => {
    if (!state) return;
    const segmentId = simulatedSegmentId++;
    const text = state.mode === "en-pt"
      ? "Estas são legendas simuladas em português. Uma tradução longa continua em blocos na ordem certa, sem perder palavras."
      : "These are simulated captions. Partial words are replaced before the final segment appears.";
    acceptCaption({ sessionId: state.sessionId, segmentId, revision: 1, mode: state.mode,
      text: state.mode === "en-en" ? "These are simulated" : text, isFinal: state.mode === "en-pt" }, true);
    if (state.mode === "en-en") acceptCaption({ sessionId: state.sessionId, segmentId, revision: 2, mode: state.mode, text, isFinal: true }, true);
  }, options);
  settingsToggle.addEventListener("click", () => toggleSettings(settings.hidden !== false), options);
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
        status.textContent = "Aplicativo desconectado";
        get("#availability").textContent = "Abra o aplicativo para acessar os controles";
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
