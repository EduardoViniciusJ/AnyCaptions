import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CaptionPresenter,
  readingTime,
} from "./captions";
import { captionWords, paginateCaption } from "./caption-layout";
import type { CaptionUpdate } from "./contracts";

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

function setup(mode: CaptionUpdate["mode"] = "auto", wordsPerPage = 20) {
  vi.useFakeTimers();
  const element = document.createElement("p");
  document.body.append(element);
  const presenter = new CaptionPresenter(
    element,
    undefined,
    undefined,
    (text) => captionWords(text).length <= wordsPerPage,
  );
  presenter.setContext(7, mode);
  const update = (segmentId: number, revision: number, text: string, isFinal: boolean): CaptionUpdate => ({
    sessionId: 7,
    mode,
    segmentId,
    revision,
    text,
    language: isFinal ? "en-US" : null,
    isFinal,
  });
  return { element, presenter, update };
}

describe("caption presenter", () => {
  it("marks the two trailing live words as partial", () => {
    const { element, presenter, update } = setup();
    expect(presenter.accept(update(1, 1, "what we have you're launching like the", false))).toBe(true);
    expect(element.querySelector(".caption-final")?.textContent).toBe("what we have you're launching ");
    expect(element.querySelector(".caption-partial")?.textContent).toBe("like the");
    expect(element.textContent).toBe("what we have you're launching like the");

    // Once finalized, the trailing word turns white too:
    expect(presenter.accept(update(1, 2, "what we have you're launching like the", true))).toBe(true);
    expect(element.querySelector(".caption-partial")).toBeNull();
    expect(element.textContent).toBe("what we have you're launching like the");

    expect(presenter.accept(update(1, 1, "Old words", false))).toBe(false);
    presenter.setContext(8, "auto");
    expect(element.textContent).toBe("");
    expect(presenter.accept(update(1, 3, "Late", true))).toBe(false);
  });

  it("shows only final Portuguese segments in order and keeps the last block visible", () => {
    const { element, presenter, update } = setup("translate-pt");
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

  it("fills the measured width before starting a new block without repeating old words", () => {
    const { element, presenter, update } = setup("auto", 5);
    presenter.accept(update(1, 1, "One two three four five", false));
    expect(element.textContent).toBe("One two three four five");

    presenter.accept(update(1, 2, "One two three four five six", false));
    expect(element.textContent).toBe("six");
    presenter.accept(update(1, 3, "One two three four five six seven", true));
    expect(element.textContent).toBe("six seven");

    presenter.accept(update(2, 1, "After a pause", false));
    expect(element.textContent).toBe("six seven After a pause");
    expect(element.querySelector(".caption-final")?.textContent).toBe("six seven After ");
    expect(element.querySelector(".caption-partial")?.textContent).toBe("a pause");
  });

  it("replaces a full block directly without keeping transition layers", () => {
    const { element, presenter, update } = setup("auto", 3);
    presenter.accept(update(1, 1, "One two three", false));
    presenter.accept(update(1, 2, "One two three four", false));

    expect(element.querySelector(".caption-content")?.textContent).toBe("four");
    expect(element.textContent).toBe("four");
    expect(element.dataset.transitioning).toBeUndefined();
    expect(element.dataset.previousCaption).toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
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
    const blocks = paginateCaption(text, (candidate) => captionWords(candidate).length <= 6);
    expect(blocks.length).toBeGreaterThan(1);
    expect(blocks.join(" ").replace(/\s+/gu, " ")).toBe(text);
    expect(blocks.every((block) => captionWords(block).length <= 6)).toBe(true);
    expect(readingTime("oi")).toBe(2000);
    expect(readingTime("x".repeat(200))).toBe(7000);
  });

  it("keeps a final caption through silence and appends the next segment while it fits", () => {
    const { element, presenter, update } = setup("auto", 8);
    presenter.accept(update(1, 1, "Vou pra casa", true));
    vi.advanceTimersByTime(60_000);
    expect(element.textContent).toBe("Vou pra casa");

    presenter.accept(update(2, 1, "Depois vou pra academia", false));
    expect(element.textContent).toBe("Vou pra casa Depois vou pra academia");
  });

  it("ignores a partial that temporarily goes backwards", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "The current sentence", false));
    presenter.accept(update(1, 2, "The current", false));
    expect(element.textContent).toBe("The current sentence");

    presenter.accept(update(1, 3, "The corrected sentence continues", false));
    expect(element.textContent).toBe("The corrected sentence continues");
  });

  it("reflows the current text after the available width changes", () => {
    let wordsPerPage = 5;
    vi.useFakeTimers();
    const element = document.createElement("p");
    document.body.append(element);
    const presenter = new CaptionPresenter(
      element,
      undefined,
      undefined,
      (text) => captionWords(text).length <= wordsPerPage,
    );
    presenter.setContext(7, "auto");
    presenter.accept({
      sessionId: 7,
      mode: "auto",
      segmentId: 1,
      revision: 1,
      text: "One two three four five",
      isFinal: false,
      language: null,
    });

    wordsPerPage = 3;
    presenter.refreshLayout();
    expect(element.textContent).toBe("four five");
    presenter.dispose();
  });

  it("does not wait for missing segments or show delayed finals from earlier speech", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(3, 1, "Current speech", false));
    expect(presenter.accept(update(1, 5, "Old final", true))).toBe(false);
    expect(element.textContent).toBe("Current speech");
    for (let segment = 4; segment < 150; segment++) {
      expect(presenter.accept(update(segment, 1, `Phrase ${segment} with words`, false))).toBe(true);
    }
    expect(element.textContent).toMatch(/Phrase 149 with words$/u);
    expect(captionWords(element.textContent ?? "").length).toBeLessThanOrEqual(20);
    expect(element.textContent).not.toContain("Old final");
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
