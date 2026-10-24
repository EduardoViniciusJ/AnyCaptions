// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { DictationIndicator, dictationErrorMessage } from "./dictation";
import type { DictationEvent } from "./contracts";

function indicator() {
  const root = document.createElement("div");
  root.hidden = true;
  const text = document.createElement("span");
  const elapsed = document.createElement("span");
  const showError = vi.fn();
  const subject = new DictationIndicator(root, text, elapsed, showError);
  return { root, text, elapsed, showError, subject };
}

function event(overrides: Partial<DictationEvent>): DictationEvent {
  return {
    phase: "recording",
    text: "",
    error: null,
    soundCue: false,
    activation: "hold",
    maxDurationSecs: 60,
    ...overrides,
  };
}

const disposers: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposers.splice(0)) dispose();
  vi.useRealTimers();
});

describe("indicador do ditado", () => {
  it("aparece ao gravar, mostra o texto reconhecido e some ao voltar ao ocioso", () => {
    const { root, text, subject } = indicator();
    disposers.push(() => subject.dispose());
    subject.accept(event({ phase: "recording" }));
    expect(root.hidden).toBe(false);
    // Sem texto ainda: mostra a instrução do modo segurar.
    expect(text.textContent).toContain("solte");
    subject.accept(event({ phase: "recording", text: "hello there" }));
    expect(text.textContent).toBe("hello there");
    subject.accept(event({ phase: "finalizing", text: "hello there" }));
    expect(root.dataset.phase).toBe("finalizing");
    subject.accept(event({ phase: "idle" }));
    expect(root.hidden).toBe(true);
  });

  it("conta o tempo no modo alternar e para no ocioso", () => {
    vi.useFakeTimers();
    const { elapsed, subject } = indicator();
    disposers.push(() => subject.dispose());
    subject.accept(event({ phase: "recording", activation: "toggle" }));
    vi.advanceTimersByTime(65_000);
    expect(elapsed.textContent).toBe("1:05");
    subject.accept(event({ phase: "idle", activation: "toggle" }));
    expect(elapsed.textContent).toBe("");
  });

  it("erros chegam como mensagem acionável ao voltar ao ocioso", () => {
    const { showError, subject } = indicator();
    disposers.push(() => subject.dispose());
    subject.accept(event({ phase: "recording" }));
    subject.accept(event({ phase: "idle", error: "dictation-paste" }));
    expect(showError).toHaveBeenCalledWith(dictationErrorMessage("dictation-paste"));
    // O texto literal nunca vira HTML: o indicador usa textContent.
    const { text, subject: second } = indicator();
    disposers.push(() => second.dispose());
    second.accept(event({ phase: "recording", text: "<b>bold</b>" }));
    expect(text.innerHTML).not.toContain("<b>");
  });
});
