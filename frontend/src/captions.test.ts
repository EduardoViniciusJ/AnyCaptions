import { afterEach, describe, expect, it, vi } from "vitest";
import { CaptionPresenter, readingTime, splitCaption } from "./captions";
import type { CaptionUpdate } from "./contracts";

afterEach(() => { vi.useRealTimers(); document.body.replaceChildren(); });

function setup(mode: CaptionUpdate["mode"] = "en-en") {
  vi.useFakeTimers();
  const element = document.createElement("p");
  document.body.append(element);
  const presenter = new CaptionPresenter(element);
  presenter.setContext(7, mode);
  const update = (segmentId: number, revision: number, text: string, isFinal: boolean): CaptionUpdate =>
    ({ sessionId: 7, mode, segmentId, revision, text, isFinal });
  return { element, presenter, update };
}

describe("caption presenter", () => {
  it("replaces partials, ignores older revisions and clears the prior session", () => {
    const { element, presenter, update } = setup();
    expect(presenter.accept(update(1, 1, "Hello", false))).toBe(true);
    expect(presenter.accept(update(1, 3, "Hello world", false))).toBe(true);
    expect(presenter.accept(update(1, 2, "Old words", false))).toBe(false);
    expect(element.textContent).toBe("Hello world");
    presenter.setContext(8, "en-en");
    expect(element.textContent).toBe("");
    expect(presenter.accept(update(1, 4, "Late", true))).toBe(false);
  });

  it("shows only final Portuguese segments in order", () => {
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
    expect(element.textContent).toBe("Segunda frase");
  });

  it("shows a new English partial immediately beside the previous final", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "First phrase", true));
    presenter.accept(update(2, 1, "Second", false));
    expect(element.textContent).toBe("First phrase\nSecond");
    presenter.accept(update(2, 2, "Second phrase", true));
    expect(element.textContent).toBe("First phrase\nSecond phrase");
    vi.advanceTimersByTime(readingTime("Second phrase") * 2);
    expect(element.textContent).toBe("First phrase\nSecond phrase");
  });

  it("keeps the latest final caption visible until it is replaced or cleared", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, "Persistent caption", true));
    vi.advanceTimersByTime(readingTime("Persistent caption") * 2);
    expect(element.textContent).toBe("Persistent caption");
    presenter.accept(update(2, 1, "Next caption", true));
    expect(element.textContent).toBe("Persistent caption\nNext caption");
    presenter.clear();
    expect(element.hidden).toBe(true);
  });

  it("keeps HTML-shaped text literal and releases its timer on clear", () => {
    const { element, presenter, update } = setup();
    presenter.accept(update(1, 1, '<img src=x onerror="alert(1)">', true));
    expect(element.querySelector("img")).toBeNull();
    expect(element.textContent).toContain("<img");
    presenter.clear();
    expect(vi.getTimerCount()).toBe(0);
    expect(element.hidden).toBe(true);
  });

  it("continues long translations without losing or splitting words", () => {
    const text = "Uma tradução extensa deve continuar em blocos ordenados com todas as palavras preservadas sem criar histórico infinito";
    const blocks = splitCaption(text, 18);
    expect(blocks.length).toBeGreaterThan(1);
    expect(blocks.every((block) => block.split("\n").length <= 3)).toBe(true);
    expect(blocks.join(" ").replace(/\s+/gu, " ")).toBe(text);
    expect(splitCaption(text, 18, 1).every((block) => !block.includes("\n"))).toBe(true);
    expect(readingTime("oi")).toBe(2000);
    expect(readingTime("x".repeat(200))).toBe(7000);
  });
});
