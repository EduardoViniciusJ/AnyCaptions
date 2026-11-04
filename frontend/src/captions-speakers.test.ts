import { afterEach, describe, expect, it, vi } from "vitest";
import { CaptionPresenter, speakerLabel, type CaptionFrame } from "./captions";
import { captionWords } from "./caption-layout";
import { setLocale } from "./i18n";
import type { CaptionUpdate, SpeakerSpan } from "./contracts";

afterEach(() => {
  vi.useRealTimers();
  setLocale("pt-br");
  document.body.replaceChildren();
});

/** Rótulos ocupam largura: a medida conta as palavras do texto renderizado. */
function setup(mode: CaptionUpdate["mode"] = "auto", wordsPerLine = 20) {
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
  presenter.setContext(7, mode);
  const update = (
    segmentId: number,
    revision: number,
    text: string,
    isFinal: boolean,
    speakers: SpeakerSpan[] = [],
  ): CaptionUpdate => ({
    sessionId: 7,
    mode,
    segmentId,
    revision,
    text,
    isFinal,
    language: isFinal ? "en-US" : null,
    speakers,
  });
  const last = () => frames.at(-1)!;
  return { presenter, update, frames, last };
}

const span = (speaker: number, text: string): SpeakerSpan => ({ speaker, text });

describe("speaker labels in the caption engine", () => {
  it("renders exactly as before when the update carries no speakers", () => {
    const { presenter, update, last } = setup();
    presenter.accept(update(1, 1, "what we have you're launching", false));
    expect(last().parts).toBeUndefined();
    expect(last().committedText).toBe("what we have");
    expect(last().livePartialText).toBe("you're launching");
    expect(last().sentenceSpeakers).toBeUndefined();
  });

  it("labels live words per speaker, keeps unknown speakers unlabeled and splits at turns", () => {
    const { presenter, update, last } = setup();
    // The diarizer has not covered the first words: no label is asserted.
    presenter.accept(update(1, 1, "well so", false, [span(0, "well so")]));
    expect(last().parts).toBeUndefined();
    expect(last().text).toBe("well so");

    presenter.accept(
      update(1, 2, "well so what now then", false, [
        span(1, "well so what"),
        span(2, "now then"),
      ]),
    );
    expect(last().text).toBe("well so what now then");
    expect(last().committedText).toBe("well so what");
    expect(last().livePartialText).toBe("now then");
    expect(last().parts).toEqual([
      { text: "well so what", speaker: 1, partial: false },
      { text: "now then", speaker: 2, partial: true },
    ]);

    presenter.accept(
      update(1, 3, "well so what now then.", true, [
        span(1, "well so what"),
        span(2, "now then."),
      ]),
    );
    expect(last().state).toBe("final");
    expect(last().parts).toEqual([
      { text: "well so what", speaker: 1, partial: false },
      { text: "now then.", speaker: 2, partial: false },
    ]);

    // The same speaker continuing into the next segment gets no new label.
    presenter.accept(update(2, 1, "and", false, [span(2, "and")]));
    expect(last().parts).toEqual([
      { text: "well so what", speaker: 1, partial: false },
      { text: "now then.", speaker: 2, partial: false },
      { text: "and", partial: true },
    ]);
    // A span total that does not match the caption's words is not trusted:
    // the new words stay unlabeled (the two newest are the live partial).
    presenter.accept(update(2, 2, "and then some", false, [span(3, "and then")]));
    expect(last().parts?.slice(-2)).toEqual([
      { text: "now then. and", speaker: 2, partial: false },
      { text: "then some", partial: true },
    ]);
  });

  it("counts the label width when paginating and labels the first visible word", () => {
    // Six display tokens per line: "Pessoa 1:" costs two of them.
    const { presenter, update, last } = setup("auto", 6);
    presenter.accept(
      update(1, 1, "a b c d e f g", true, [span(1, "a b c d"), span(2, "e f g")]),
    );
    expect(last().text).toBe("e f g");
    expect(last().parts).toEqual([{ text: "e f g", speaker: 2, partial: false }]);
    // Without labels the same words would have fit on one line.
    const plain = setup("auto", 6);
    plain.presenter.accept(plain.update(1, 1, "a b c d e f", true));
    expect(plain.last().text).toBe("a b c d e f");
  });

  it("labels translated sentences by the speaker of their source span", () => {
    const { presenter, update, last } = setup("translate-pt-br");
    const sentence = (segment: number, text: string, speakers: SpeakerSpan[]) =>
      update(segment, 1, text, true, speakers);
    presenter.accept(
      sentence(1, "Você conseguiu terminar?", [span(1, "Você conseguiu terminar?")]),
    );
    presenter.accept(sentence(2, "Sim, terminei agora.", [span(2, "Sim, terminei agora.")]));
    presenter.accept(sentence(3, "E funciona bem.", [span(2, "E funciona bem.")]));
    expect(last().sentences).toEqual([
      "Você conseguiu terminar?",
      "Sim, terminei agora.",
      "E funciona bem.",
    ]);
    expect(last().sentenceSpeakers).toEqual([1, 2, undefined]);
    expect(last().lines).toBe("multi");

    // A sentence without a speaker span keeps flowing, unlabeled.
    presenter.accept(sentence(4, "Ok.", []));
    expect(last().sentenceSpeakers).toEqual([1, 2, undefined, undefined]);

    // Translation without labels never sets sentence speakers.
    const plain = setup("translate-pt-br");
    plain.presenter.accept(plain.update(1, 1, "Olá.", true));
    expect(plain.last().sentenceSpeakers).toBeUndefined();
  });

  it("puts each speaker turn on its own line while labels are on, and only then", () => {
    const { presenter, update, last } = setup("auto", 8);
    presenter.setSpeakerLabels(true);
    presenter.accept(
      update(1, 1, "embaixo também tripla tá disparo sim", true, [
        span(1, "embaixo também"),
        span(2, "tripla"),
        span(3, "tá disparo sim"),
      ]),
    );
    expect(last().lines).toBe("multi");
    expect(last().parts).toEqual([
      { text: "embaixo também", speaker: 1, partial: false },
      { text: "tripla", speaker: 2, partial: false },
      { text: "tá disparo sim", speaker: 3, partial: false },
    ]);
    // A fourth turn no longer fits the three lines: the oldest turn leaves whole.
    presenter.accept(update(2, 1, "é por causa do vale", false, [span(1, "é por causa do vale")]));
    expect(last().parts?.map((part) => part.speaker ?? "-")).toEqual([2, 3, 1, "-"]);
    expect(last().text).toBe("tripla tá disparo sim é por causa do vale");
    // Unknown speaker words form an unlabeled turn in this mode, still on multi lines.
    const unknown = setup("auto", 8);
    unknown.presenter.setSpeakerLabels(true);
    unknown.presenter.accept(unknown.update(1, 1, "well so", false, [span(0, "well so")]));
    expect(unknown.last().lines).toBe("multi");
    expect(unknown.last().parts).toEqual([{ text: "well so", partial: true }]);
    // Turning the mode off restores the single line immediately.
    unknown.presenter.setSpeakerLabels(false);
    expect(unknown.last().lines).toBe("single");
    expect(unknown.last().parts).toBeUndefined();
  });

  it("breaks the translated flow per speaker only while labels are on", () => {
    const { presenter, update, last } = setup("translate-pt-br", 6);
    presenter.setSpeakerLabels(true);
    const sentence = (segment: number, speaker: number, text: string) =>
      update(segment, 1, text, true, [span(speaker, text)]);
    presenter.accept(sentence(1, 1, "Você conseguiu terminar?"));
    presenter.accept(sentence(2, 2, "Sim, terminei agora."));
    presenter.accept(sentence(3, 2, "E funciona bem."));
    expect(last().sentenceSpeakers).toEqual([1, 2, undefined]);
    expect(last().sentences.length).toBe(3);
    // Each turn counts its own lines: a long fourth turn pushes the oldest text out.
    presenter.accept(sentence(4, 3, "Provavelmente não, só terminar o trabalho."));
    expect(last().sentences).toEqual([
      "E funciona bem.",
      "Provavelmente não, só terminar o trabalho.",
    ]);
    expect(last().sentenceSpeakers?.at(-1)).toBe(3);
  });

  it("formats the generic label in the interface language", () => {
    expect(speakerLabel(1)).toBe("Pessoa 1:");
    setLocale("en");
    expect(speakerLabel(2)).toBe("Person 2:");
    setLocale("es");
    expect(speakerLabel(3)).toBe("Persona 3:");
  });
});
