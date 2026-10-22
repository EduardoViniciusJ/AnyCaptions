import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CAPTION_FADE_MS,
  CaptionPresenter,
  LIVE_IDLE_CLEAR_MS,
  LIVE_LINES,
  readingTime,
  splitCaption,
  wrapLines,
} from "./captions";
import type { CaptionUpdate } from "./contracts";

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

// Without layout (jsdom), live lines hold 42 characters.
function setup(mode: CaptionUpdate["mode"] = "en-en") {
  vi.useFakeTimers();
  const element = document.createElement("p");
  document.body.append(element);
  const presenter = new CaptionPresenter(element);
  presenter.setContext(7, mode);
  const update = (segmentId: number, revision: number, text: string, isFinal: boolean): CaptionUpdate => ({
    sessionId: 7,
    mode,
    segmentId,
    revision,
    text,
    isFinal,
  });
  return { element, presenter, update };
}

describe("caption presenter", () => {
  it("shows each revision immediately and lets the next one correct it", () => {
    const { element, presenter, update } = setup();
    expect(presenter.accept(update(1, 1, "what we have you're", false))).toBe(true);
    expect(element.textContent).toBe("what we have you're");

    // A later revision replaces the words it corrects.
    expect(presenter.accept(update(1, 2, "what we have your launching", false))).toBe(true);
    expect(element.textContent).toBe("what we have your launching");

    expect(presenter.accept(update(1, 3, "What we have, you're launching like the", true))).toBe(true);
    expect(element.textContent).toBe("What we have, you're launching like the");

    expect(presenter.accept(update(1, 1, "Old words", false))).toBe(false);
    presenter.setContext(8, "en-en");
    expect(element.textContent).toBe("");
    expect(presenter.accept(update(1, 3, "Late", true))).toBe(false);
  });

  it("keeps writing after pauses and rolls up one line when the bottom line fills", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "eu vou", true));
    // Nobody speaks for a moment: the words stay on screen.
    vi.advanceTimersByTime(LIVE_IDLE_CLEAR_MS - 1000);
    expect(element.textContent).toBe("eu vou");

    // The next phrase continues after it on the same line.
    presenter.accept(update(2, 1, "para casa e depois", true));
    expect(element.textContent).toBe("eu vou para casa e depois");

    // Filling more than LIVE_LINES lines moves the text up instead of clearing it.
    presenter.accept(update(3, 1, "nós vamos jantar juntos com a família inteira no restaurante", true));
    presenter.accept(update(4, 1, "e depois voltamos caminhando pela praia", true));
    const lines = element.textContent!.split("\n");
    expect(lines).toHaveLength(LIVE_LINES);
    expect(lines.every((line) => line.length <= 42)).toBe(true);
    expect(element.textContent).not.toContain("eu vou");
    expect(element.textContent!.endsWith("praia")).toBe(true);
  });

  it("fades in only the words added by the latest update", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "hello there", true));
    presenter.accept(update(2, 1, "general", true));
    const fresh = [...element.querySelectorAll(".caption-word-new")].map((span) => span.textContent);
    expect(fresh).toEqual(["general"]);
  });

  it("wraps words greedily so earlier lines never change", () => {
    expect(wrapLines([], 10)).toEqual([]);
    expect(wrapLines(["one", "two", "three"], 7)).toEqual([["one", "two"], ["three"]]);
    expect(wrapLines(["one", "two", "three", "four"], 7)).toEqual([["one", "two"], ["three"], ["four"]]);
    // A word longer than the line still gets its own line.
    expect(wrapLines(["extraordinarily"], 5)).toEqual([["extraordinarily"]]);
  });

  it("shows only final Portuguese segments in order and keeps the last block visible", () => {
    const { element, presenter, update } = setup("en-pt");
    expect(presenter.accept(update(1, 1, "English partial", false))).toBe(false);
    expect(element.textContent).toBe("");
    expect(presenter.accept(update(2, 1, "Segunda frase", true))).toBe(true);
    expect(element.textContent).toBe("");
    expect(presenter.accept(update(1, 1, "Primeira frase", true))).toBe(true);
    expect(element.textContent).toBe("Primeira frase");
    vi.advanceTimersByTime(readingTime("Primeira frase"));
    expect(element.textContent).toBe("Segunda frase");
    vi.advanceTimersByTime(readingTime("Segunda frase") * 2);
    expect(element.hidden).toBe(false);
    expect(element.textContent).toBe("Segunda frase");
  });

  it("fades the text out after a long silence and starts on an empty box afterwards", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "Short line", true));
    vi.advanceTimersByTime(LIVE_IDLE_CLEAR_MS - 1);
    expect(element.textContent).toBe("Short line");
    vi.advanceTimersByTime(1);
    // The text stays while it fades, then is removed.
    expect(element.style.opacity).toBe("0");
    expect(element.textContent).toBe("Short line");
    vi.advanceTimersByTime(CAPTION_FADE_MS);
    expect(element.hidden).toBe(true);

    presenter.accept(update(2, 1, "new speech", true));
    expect(element.textContent).toBe("new speech");
  });

  it("a new update cancels the pending fade-out", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "keep going", true));
    vi.advanceTimersByTime(LIVE_IDLE_CLEAR_MS - 10);
    presenter.accept(update(2, 1, "now", true));
    vi.advanceTimersByTime(LIVE_IDLE_CLEAR_MS - 10);
    expect(element.hidden).toBe(false);
    expect(element.textContent).toBe("keep going now");
  });

  it("keeps HTML-shaped text literal and fades before clearing", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, '<img src=x onerror="alert(1)">', true));
    expect(element.querySelector("img")).toBeNull();
    expect(element.textContent).toContain("<img");
    presenter.clear();
    expect(element.hidden).toBe(false);
    expect(element.style.opacity).toBe("0");
    vi.advanceTimersByTime(CAPTION_FADE_MS);
    expect(vi.getTimerCount()).toBe(0);
    expect(element.hidden).toBe(true);
  });

  it("continues long translations without losing or splitting words", () => {
    const text =
      "Uma tradução extensa deve continuar em blocos ordenados com todas as palavras preservadas sem criar histórico infinito";
    const blocks = splitCaption(text, 18);
    expect(blocks.length).toBeGreaterThan(1);
    expect(blocks.every((block) => block.split("\n").length <= 2)).toBe(true);
    expect(blocks.join(" ").replace(/\s+/gu, " ")).toBe(text);
    expect(splitCaption(text, 18, 1).every((block) => !block.includes("\n"))).toBe(true);
    expect(readingTime("oi")).toBe(2000);
    expect(readingTime("x".repeat(200))).toBe(7000);
  });

  it("does not show delayed finals from earlier speech and stays bounded", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(3, 1, "Current speech", true));
    expect(presenter.accept(update(1, 5, "Old final", true))).toBe(false);
    expect(element.textContent).toBe("Current speech");
    for (let segment = 4; segment < 400; segment++) {
      expect(presenter.accept(update(segment, 1, `Phrase ${segment} with words`, true))).toBe(true);
    }
    expect(element.textContent!.split("\n")).toHaveLength(LIVE_LINES);
    expect(element.textContent!.endsWith("words")).toBe(true);
    expect(element.textContent).toContain("399");
    expect(element.textContent).not.toContain("Old final");
  });

  it("cancels an outgoing fade when new speech arrives", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "First phrase", true));
    presenter.clear();
    expect(element.style.opacity).toBe("0");
    presenter.accept(update(2, 1, "New speech", true));
    vi.advanceTimersByTime(CAPTION_FADE_MS);
    expect(element.textContent).toBe("New speech");
    expect(element.style.opacity).toBe("1");
    expect(element.hidden).toBe(false);
    presenter.dispose();
    expect(vi.getTimerCount()).toBe(0);
    expect(element.hidden).toBe(true);
  });

  it("clears immediately when reduced motion is requested", () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true }));
    try {
      const { element, presenter, update } = setup();
      presenter.accept(update(1, 1, "Some speech", true));
      presenter.clear();
      expect(element.hidden).toBe(true);
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
