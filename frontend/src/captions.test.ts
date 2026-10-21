import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CaptionPresenter,
  liveCaptionTime,
  readingTime,
  splitCaption,
  splitPhraseChunks,
} from "./captions";
import type { CaptionUpdate } from "./contracts";

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

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
  it("renders leading words in white and only the trailing word in gray italic during live speech", () => {
    const { element, presenter, update } = setup();
    expect(presenter.accept(update(1, 1, "what we have you're launching like the", false))).toBe(true);
    expect(element.querySelector(".caption-final")?.textContent).toBe("what we have you're launching like ");
    expect(element.querySelector(".caption-partial")?.textContent).toBe("the");
    expect(element.textContent).toBe("what we have you're launching like the");

    // Once finalized, the trailing word turns white too:
    expect(presenter.accept(update(1, 2, "what we have you're launching like the", true))).toBe(true);
    expect(element.querySelector(".caption-partial")).toBeNull();
    expect(element.textContent).toBe("what we have you're launching like the");

    expect(presenter.accept(update(1, 1, "Old words", false))).toBe(false);
    presenter.setContext(8, "en-en");
    expect(element.textContent).toBe("");
    expect(presenter.accept(update(1, 3, "Late", true))).toBe(false);
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

  it("never leaves a 1-2 word orphan like 'the' when rolling past 9 words", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "One two three four five six seven eight nine", false));
    expect(element.textContent).toBe("One two three four five six seven eight nine");

    // Adding the 10th word keeps a 5-word sliding window instead of showing "the" alone!
    presenter.accept(update(1, 2, "One two three four five six seven eight nine the", false));
    expect(element.textContent).toBe("six seven eight nine the");
    expect(element.querySelector(".caption-final")?.textContent).toBe("six seven eight nine ");
    expect(element.querySelector(".caption-partial")?.textContent).toBe("the");
  });

  it("keeps HTML-shaped text literal and fades before clearing", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, '<img src=x onerror="alert(1)">', true));
    expect(element.querySelector("img")).toBeNull();
    expect(element.textContent).toContain("<img");
    presenter.clear();
    expect(element.hidden).toBe(false);
    expect(element.style.opacity).toBe("0");
    vi.advanceTimersByTime(120);
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
    expect(splitPhraseChunks(text, 64, 9).every((chunk) => chunk.length <= 64)).toBe(true);
    expect(readingTime("oi")).toBe(2000);
    expect(readingTime("x".repeat(200))).toBe(7000);
    expect(liveCaptionTime("oi")).toBe(2000);
    expect(liveCaptionTime("x".repeat(200))).toBe(5000);
  });

  it("does not wait for missing segments or show delayed finals from earlier speech", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(3, 1, "Current speech", false));
    expect(presenter.accept(update(1, 5, "Old final", true))).toBe(false);
    expect(element.textContent).toBe("Current speech");
    for (let segment = 4; segment < 150; segment++) {
      expect(presenter.accept(update(segment, 1, `Phrase ${segment} with words`, false))).toBe(true);
    }
    expect(element.textContent).toBe("Phrase 149 with words");
  });

  it("cancels an outgoing fade when a new partial arrives", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "First phrase", true));
    presenter.clear();
    expect(element.style.opacity).toBe("0");
    presenter.accept(update(2, 1, "New speech", false));
    vi.advanceTimersByTime(120);
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
