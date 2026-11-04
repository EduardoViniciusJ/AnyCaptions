import { afterEach, describe, expect, it, vi } from "vitest";
import { CaptionPresenter, type CaptionFrame } from "./captions";
import { captionWords } from "./caption-layout";
import type { CaptionUpdate, SpeakerSpan } from "./contracts";

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

function setup(wordsPerLine = 20) {
  vi.useFakeTimers();
  const element = document.createElement("p");
  document.body.append(element);
  const frames: CaptionFrame[] = [];
  const presenter = new CaptionPresenter(
    element,
    undefined,
    (frame) => frames.push(frame),
    (text, maxLines = 1) => captionWords(text).length <= wordsPerLine * maxLines,
  );
  presenter.setContext(7, "translate-pt-br");
  const provisional = (
    segmentId: number,
    revision: number,
    text: string,
    speakers: SpeakerSpan[] = [],
  ): CaptionUpdate => ({
    sessionId: 7,
    mode: "translate-pt-br",
    segmentId,
    revision,
    text,
    isFinal: false,
    language: "pt-br",
    speakers,
    provisional: true,
  });
  const sentence = (
    segmentId: number,
    revision: number,
    text: string,
    isFinal = true,
    speakers: SpeakerSpan[] = [],
  ): CaptionUpdate => ({
    sessionId: 7,
    mode: "translate-pt-br",
    segmentId,
    revision,
    text,
    isFinal,
    language: "pt-br",
    speakers,
  });
  const last = () => frames.at(-1)!;
  return { presenter, provisional, sentence, last };
}

describe("live translation", () => {
  it("shows the provisional translation after the closed sentences until the final replaces it", () => {
    const { presenter, provisional, sentence, last } = setup();
    presenter.accept(sentence(1, 1, "Primeira frase."));
    expect(presenter.accept(provisional(2, 3, "Você conseguiu"))).toBe(true);
    expect(last().sentences).toEqual(["Primeira frase."]);
    expect(last().provisionalText).toBe("Você conseguiu");
    expect(last().text).toBe("Primeira frase. Você conseguiu");
    // A newer interim of the same segment replaces the provisional text.
    expect(presenter.accept(provisional(2, 5, "Você conseguiu terminar"))).toBe(true);
    expect(last().provisionalText).toBe("Você conseguiu terminar");
    // Stale interims are ignored.
    expect(presenter.accept(provisional(2, 4, "Você"))).toBe(false);
    // The final translation arrives with its own (lower) revision numbers:
    // it supersedes the provisional text instead of being dropped as stale.
    expect(presenter.accept(sentence(2, 1, "Você conseguiu terminar?", false))).toBe(true);
    expect(presenter.accept(sentence(2, 2, "Sim, agora.", true))).toBe(true);
    expect(last().provisionalText).toBeUndefined();
    expect(last().sentences).toEqual(["Primeira frase.", "Você conseguiu terminar?", "Sim, agora."]);
    // After the final, nothing else of that segment is accepted.
    expect(presenter.accept(provisional(2, 9, "tarde demais"))).toBe(false);
  });

  it("keeps the provisional text when older sentences overflow and labels its speaker", () => {
    // Four display tokens per line, three lines; with labels on, each
    // speaker turn counts its own lines ("Pessoa N:" is two tokens).
    const { presenter, provisional, sentence, last } = setup(4);
    presenter.setSpeakerLabels(true);
    presenter.accept(sentence(1, 1, "Um dois.", true, [{ speaker: 1, text: "Um dois." }]));
    presenter.accept(sentence(2, 1, "Três quatro.", true, [{ speaker: 2, text: "Três quatro." }]));
    presenter.accept(provisional(3, 2, "cinco seis sete", [{ speaker: 3, text: "cinco seis sete" }]));
    // The oldest turn leaves; the provisional one stays and gets the new
    // speaker's label.
    expect(last().sentences).toEqual(["Três quatro."]);
    expect(last().provisionalText).toBe("cinco seis sete");
    expect(last().provisionalSpeaker).toBe(3);
    // Same speaker as the previous sentence: no label on the provisional text.
    presenter.accept(provisional(3, 3, "cinco seis", [{ speaker: 2, text: "cinco seis" }]));
    expect(last().provisionalSpeaker).toBeUndefined();
  });

  it("starts a new utterance's provisional text even before the previous final and clears on reset", () => {
    const { presenter, provisional, sentence, last } = setup();
    presenter.accept(provisional(1, 2, "Primeira em andamento"));
    presenter.accept(provisional(2, 1, "Segunda em andamento"));
    expect(last().sentences).toEqual([]);
    expect(last().provisionalText).toBe("Segunda em andamento");
    presenter.accept(sentence(2, 1, "Segunda fechada."));
    expect(last().provisionalText).toBeUndefined();
    expect(last().sentences).toEqual(["Segunda fechada."]);
    presenter.clear(true);
    expect(last().text).toBe("");
    expect(last().provisionalText).toBeUndefined();
  });
});
