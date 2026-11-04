import type { CaptionMode, CaptionUpdate } from "./contracts";
import {
  CaptionMeasurer,
  captionWords,
  paginateCaption,
  type CaptionFits,
} from "./caption-layout";
import { t } from "./i18n";

export interface CaptionClock {
  set(callback: () => void, delay: number): ReturnType<typeof setTimeout>;
  clear(handle: ReturnType<typeof setTimeout>): void;
}

const browserClock: CaptionClock = {
  set: (callback, delay) => setTimeout(callback, delay),
  clear: (handle) => clearTimeout(handle),
};

const CLEAR_FADE_MS = 120;

// Modo frases (tradutor rápido): o texto flui preenchendo a largura, quebrando
// em até este número de linhas; estourou, a sentença mais antiga sai inteira.
// Espelhado no CSS de `.caption[data-lines="multi"]`.
const SENTENCE_LINES = 3;

/** Rótulo genérico de um slot do diarizador ("Pessoa 1:"), no idioma da interface. */
export function speakerLabel(speaker: number): string {
  return `${t("speakerLabel").replace("{n}", String(speaker))}:`;
}

/** Uma palavra ao vivo com o slot de falante (0 = ainda indefinido). */
interface LiveWord {
  text: string;
  speaker: number;
}

/** Uma sentença traduzida com o falante do trecho de origem (0 = sem rótulo). */
interface Sentence {
  text: string;
  speaker: number;
}

/**
 * Palavras de uma atualização com o falante de cada uma. Os trechos do backend
 * só são confiados quando suas palavras somam exatamente as da legenda; caso
 * contrário (ou sem rótulos) todas ficam sem falante.
 */
function wordsOf(update: CaptionUpdate): LiveWord[] {
  const plain = captionWords(update.text);
  const spans = update.speakers ?? [];
  if (spans.length === 0) return plain.map((text) => ({ text, speaker: 0 }));
  const tagged = spans.flatMap((span) =>
    captionWords(span.text).map((text) => ({ text, speaker: span.speaker })),
  );
  if (tagged.length !== plain.length)
    return plain.map((text) => ({ text, speaker: 0 }));
  return tagged;
}

/**
 * Em que índices entra um rótulo: no início de cada turno de um falante
 * conhecido. Palavras sem falante continuam o turno anterior, nunca o abrem.
 */
function labelStarts(speakers: readonly number[]): Set<number> {
  const starts = new Set<number>();
  let current = 0;
  speakers.forEach((speaker, index) => {
    if (speaker > 0 && speaker !== current) {
      starts.add(index);
      current = speaker;
    }
  });
  return starts;
}

/** Texto como será exibido (com rótulos), para medir a largura. */
function renderWords(words: readonly LiveWord[]): string {
  const starts = labelStarts(words.map((word) => word.speaker));
  return words
    .map((word, index) =>
      starts.has(index) ? `${speakerLabel(word.speaker)} ${word.text}` : word.text,
    )
    .join(" ");
}

function renderSentences(sentences: readonly Sentence[]): string {
  const starts = labelStarts(sentences.map((sentence) => sentence.speaker));
  return sentences
    .map((sentence, index) =>
      starts.has(index)
        ? `${speakerLabel(sentence.speaker)} ${sentence.text}`
        : sentence.text,
    )
    .join(" ");
}

/**
 * Pagina itens por largura: a maior fatia que cabe, repetidamente. Com a
 * renderização sem rótulos equivale a `paginateCaption` sobre as palavras.
 */
function paginateItems<T>(
  items: readonly T[],
  render: (slice: readonly T[]) => string,
  fits: CaptionFits,
): T[][] {
  const pages: T[][] = [];
  let start = 0;
  while (start < items.length) {
    let lower = start + 1;
    let upper = items.length;
    let end = lower;
    while (lower <= upper) {
      const middle = Math.floor((lower + upper) / 2);
      if (fits(render(items.slice(start, middle)))) {
        end = middle;
        lower = middle + 1;
      } else {
        upper = middle - 1;
      }
    }
    pages.push(items.slice(start, end));
    start = end;
  }
  return pages;
}

/** Um trecho visível da legenda ao vivo; `speaker` presente = rótulo antes dele. */
export interface CaptionPart {
  text: string;
  speaker?: number;
  partial: boolean;
}

/** Um turno de fala: as palavras consecutivas de um falante (0 = desconhecido). */
interface Turn<T> {
  speaker: number;
  items: T[];
}

function turnsOf<T>(items: readonly T[], speakerOf: (item: T) => number): Turn<T>[] {
  const starts = labelStarts(items.map(speakerOf));
  const turns: Turn<T>[] = [];
  items.forEach((item, index) => {
    if (index === 0 || starts.has(index)) {
      turns.push({ speaker: speakerOf(item), items: [item] });
    } else {
      turns.at(-1)!.items.push(item);
    }
  });
  return turns;
}

/**
 * Com rótulos ligados, cada turno ocupa a própria linha (ou linhas). Mede as
 * linhas que um turno precisa, do mais curto ao limite; acima disso devolve
 * `limit + 1`.
 */
function linesNeeded(text: string, fits: CaptionFits, limit: number): number {
  for (let lines = 1; lines <= limit; lines++) {
    if (fits(text, lines)) return lines;
  }
  return limit + 1;
}

/**
 * Mantém os turnos mais recentes que cabem em `maxLines` linhas, um turno por
 * linha; devolve quantos turnos do início saíram. Um único turno maior que a
 * área fica inteiro (quem chama corta sua cauda).
 */
function keepRecentTurns(
  rendered: readonly string[],
  fits: CaptionFits,
  maxLines: number,
): number {
  let used = 0;
  for (let index = rendered.length - 1; index >= 0; index--) {
    used += linesNeeded(rendered[index]!, fits, maxLines);
    if (used > maxLines) return index === rendered.length - 1 ? index : index + 1;
  }
  return 0;
}

export interface CaptionFrame {
  text: string;
  committedText?: string;
  livePartialText?: string;
  sentences: string[];
  /**
   * Modo ao vivo com rótulos de falante: as palavras visíveis agrupadas por
   * turno. Ausente quando nenhuma palavra visível tem falante conhecido, e a
   * legenda é renderizada exatamente como sem o recurso.
   */
  parts?: CaptionPart[];
  /** Modo tradução: falante rotulado antes de cada sentença (alinhado a `sentences`). */
  sentenceSpeakers?: (number | undefined)[];
  animateNewest: boolean;
  lines: "multi" | "single";
  state: "final" | "partial";
  opacity: number;
}

interface Block {
  text: string;
  committedText?: string;
  livePartialText?: string;
  parts?: CaptionPart[];
  segmentId: number;
}

interface ActiveLiveSegment {
  words: LiveWord[];
  consumedWords: number;
}

/**
 * Apresenta as legendas ao vivo em todos os modos: cada revisão substitui o
 * texto imediatamente, sem fila nem ritmo de leitura. No modo tradução as
 * revisões chegam do estágio de tradução já em português.
 */
export class CaptionPresenter {
  private sessionId = 0;
  private mode: CaptionMode = "auto";
  /**
   * Rótulos de falante ativos: cada turno de fala ocupa a própria linha
   * ("Pessoa 1: …" / "Pessoa 2: …"). Desligado, a legenda ao vivo é a linha
   * única de sempre.
   */
  private speakerLabelsMode = false;
  private sentences: Sentence[] = [];
  /** Anima (fade 120 ms) só a sentença recém-chegada no próximo render. */
  private animateNewestSentence = false;
  private activeSegmentId = 0;
  private activeRevision = -1;
  private activeFinal = false;
  private finalizedLiveWords: LiveWord[] = [];
  private activeLiveSegment: ActiveLiveSegment | undefined;
  private current: Block | undefined;
  private displayedText = "";
  private lastFrame: CaptionFrame | undefined;
  private fadeTimer: ReturnType<typeof setTimeout> | undefined;
  private readonly measurer: CaptionMeasurer | undefined;
  private readonly fitsCaption: CaptionFits;

  constructor(
    element: HTMLElement,
    private readonly clock: CaptionClock = browserClock,
    private readonly onChange: (frame: CaptionFrame) => void = () => {},
    fitsCaption?: CaptionFits,
  ) {
    this.measurer = fitsCaption ? undefined : new CaptionMeasurer(element);
    this.fitsCaption = fitsCaption ?? this.measurer!.fits;
  }

  setContext(sessionId: number, mode: CaptionMode): void {
    if (sessionId === this.sessionId && mode === this.mode) return;
    this.clear(true);
    this.sessionId = sessionId;
    this.mode = mode;
  }

  /** Liga o layout por turnos (um falante por linha); só muda algo quando ativo. */
  setSpeakerLabels(enabled: boolean): void {
    if (enabled === this.speakerLabelsMode) return;
    this.speakerLabelsMode = enabled;
    this.refreshLayout();
  }

  accept(update: CaptionUpdate): boolean {
    if (
      update.sessionId !== this.sessionId ||
      update.mode !== this.mode ||
      !Number.isSafeInteger(update.segmentId) ||
      update.segmentId < 1
    )
      return false;
    if (
      !Number.isSafeInteger(update.revision) ||
      update.revision < 0 ||
      update.text.length > 4000
    )
      return false;
    if (
      update.segmentId < this.activeSegmentId ||
      (update.segmentId === this.activeSegmentId &&
        (this.activeFinal || update.revision <= this.activeRevision))
    )
      return false;

    const newSegment = update.segmentId !== this.activeSegmentId;
    if (newSegment) this.commitActiveLiveSegment();

    this.activeSegmentId = update.segmentId;
    this.activeRevision = update.revision;
    this.activeFinal = update.isFinal;

    // Translate mode: each update is one translated sentence, shown
    // immediately. The text flows over up to SENTENCE_LINES lines and, once
    // full, the screen clears and restarts with the newest sentence.
    if (this.mode !== "auto") {
      const sentence = update.text.trim();
      if (sentence) {
        // A translated sentence belongs to one speaker (the stage translates
        // each speaker's span on its own); several spans mean no safe label.
        const spans = update.speakers ?? [];
        const speaker = spans.length === 1 ? (spans[0]?.speaker ?? 0) : 0;
        this.sentences.push({ text: sentence, speaker });
        this.animateNewestSentence = true;
      }
      this.layoutSentences();
      this.render();
      return true;
    }

    const incomingWords = wordsOf(update);
    const previous = this.activeLiveSegment;
    const previousWords = previous?.words ?? [];
    const regressivePartial =
      !update.isFinal &&
      incomingWords.length < previousWords.length &&
      incomingWords.every(
        (word, index) => word.text === previousWords[index]?.text,
      );

    this.activeLiveSegment = {
      words: regressivePartial ? previousWords : incomingWords,
      consumedWords: newSegment ? 0 : (previous?.consumedWords ?? 0),
    };

    this.layoutLiveCaption();
    if (update.isFinal) this.commitActiveLiveSegment();
    this.render();
    return true;
  }

  /** Cabe nas linhas do modo frases? Com rótulos, cada turno conta as próprias linhas. */
  private sentencesFit(sentences: readonly Sentence[]): boolean {
    if (!this.speakerLabelsMode) {
      return this.fitsCaption(renderSentences(sentences), SENTENCE_LINES);
    }
    const rendered = turnsOf(sentences, (sentence) => sentence.speaker).map((turn) =>
      renderSentences(turn.items),
    );
    return keepRecentTurns(rendered, this.fitsCaption, SENTENCE_LINES) === 0;
  }

  private layoutSentences(): void {
    const fitsLines = (text: string) => this.fitsCaption(text, SENTENCE_LINES);
    if (this.sentences.length > 1 && !this.sentencesFit(this.sentences)) {
      // Encheu as linhas: a tela limpa e recomeça do zero com a sentença nova,
      // em vez de ir empurrando as antigas para cima.
      this.sentences = this.sentences.slice(-1);
    }
    let text = this.sentences.map((sentence) => sentence.text).join(" ");
    if (text && !fitsLines(renderSentences(this.sentences))) {
      // A single sentence taller than the area keeps only its tail.
      const speaker = this.sentences[0]?.speaker ?? 0;
      text = paginateCaption(text, fitsLines).at(-1) ?? text;
      this.sentences = [{ text, speaker }];
    }
    this.current = text ? { text, segmentId: this.activeSegmentId } : undefined;
  }

  private liveWords(): LiveWord[] {
    const activeWords = this.activeLiveSegment
      ? this.activeLiveSegment.words.slice(this.activeLiveSegment.consumedWords)
      : [];
    return [...this.finalizedLiveWords, ...activeWords];
  }

  private commitActiveLiveSegment(): void {
    if (!this.activeLiveSegment) return;
    this.finalizedLiveWords.push(
      ...this.activeLiveSegment.words.slice(
        this.activeLiveSegment.consumedWords,
      ),
    );
    this.activeLiveSegment = undefined;
  }

  private consumeLiveWords(count: number): void {
    const finalized = Math.min(count, this.finalizedLiveWords.length);
    this.finalizedLiveWords.splice(0, finalized);
    const active = count - finalized;
    if (active > 0 && this.activeLiveSegment) {
      this.activeLiveSegment.consumedWords = Math.min(
        this.activeLiveSegment.words.length,
        this.activeLiveSegment.consumedWords + active,
      );
    }
  }

  private layoutLiveCaption(): void {
    const visible = this.speakerLabelsMode
      ? this.visibleLiveTurns()
      : this.visibleLiveLine();
    const activeVisibleWords = this.activeLiveSegment
      ? this.activeLiveSegment.words.length -
        this.activeLiveSegment.consumedWords
      : 0;
    const trailing = this.activeFinal ? 0 : Math.min(2, activeVisibleWords);
    const splitAt = Math.max(0, visible.length - trailing);
    const plain = (words: readonly LiveWord[]) =>
      words.map((word) => word.text).join(" ");
    const labeled =
      this.speakerLabelsMode || visible.some((word) => word.speaker > 0);
    this.current = visible.length
      ? {
          text: plain(visible),
          committedText: plain(visible.slice(0, splitAt)),
          livePartialText: plain(visible.slice(splitAt)),
          parts: labeled ? partsOf(visible, splitAt) : undefined,
          segmentId: this.activeSegmentId,
        }
      : undefined;
  }

  /** Linha única (rótulos desligados): a última página que cabe na largura. */
  private visibleLiveLine(): LiveWord[] {
    const pages = paginateItems(this.liveWords(), renderWords, this.fitsCaption);
    const discardedWords = pages
      .slice(0, -1)
      .reduce((total, page) => total + page.length, 0);
    if (discardedWords > 0) this.consumeLiveWords(discardedWords);
    return pages.at(-1) ?? [];
  }

  /**
   * Um falante por linha (rótulos ligados): os turnos mais recentes que cabem
   * nas linhas da área; os anteriores saem inteiros. Um turno sozinho maior
   * que a área mostra só a cauda.
   */
  private visibleLiveTurns(): LiveWord[] {
    const words = this.liveWords();
    const turns = turnsOf(words, (word) => word.speaker);
    const rendered = turns.map((turn) => renderWords(turn.items));
    const dropped = keepRecentTurns(rendered, this.fitsCaption, SENTENCE_LINES);
    let discarded = turns
      .slice(0, dropped)
      .reduce((total, turn) => total + turn.items.length, 0);
    let visible = words.slice(discarded);
    if (
      turns.length - dropped === 1 &&
      linesNeeded(rendered.at(-1)!, this.fitsCaption, SENTENCE_LINES) >
        SENTENCE_LINES
    ) {
      const fitsLines = (text: string) => this.fitsCaption(text, SENTENCE_LINES);
      const pages = paginateItems(visible, renderWords, fitsLines);
      discarded += pages
        .slice(0, -1)
        .reduce((total, page) => total + page.length, 0);
      visible = pages.at(-1) ?? [];
    }
    if (discarded > 0) this.consumeLiveWords(discarded);
    return visible;
  }

  refreshLayout(): void {
    if (this.mode !== "auto") {
      if (this.sentences.length === 0) return;
      this.layoutSentences();
      this.render();
      return;
    }
    if (this.liveWords().length === 0) return;
    this.layoutLiveCaption();
    this.render();
  }

  private render(immediate = false): void {
    const reducedMotion =
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    if (!this.current && !immediate && !reducedMotion && this.displayedText) {
      if (this.fadeTimer !== undefined) return;
      if (this.lastFrame) this.onChange({ ...this.lastFrame, opacity: 0 });
      this.fadeTimer = this.clock.set(() => {
        this.fadeTimer = undefined;
        this.render(true);
      }, CLEAR_FADE_MS);
      return;
    }
    if (this.fadeTimer !== undefined) this.clock.clear(this.fadeTimer);
    this.fadeTimer = undefined;
    this.displayedText = this.current?.text ?? "";
    const sentenceMode = this.mode !== "auto";
    const sentenceStarts = labelStarts(
      this.sentences.map((sentence) => sentence.speaker),
    );
    this.lastFrame = {
      text: this.displayedText,
      committedText: this.current?.committedText,
      livePartialText: this.current?.livePartialText,
      parts: this.current?.parts,
      sentences: sentenceMode
        ? this.sentences.map((sentence) => sentence.text)
        : [],
      sentenceSpeakers:
        sentenceMode && sentenceStarts.size > 0
          ? this.sentences.map((sentence, index) =>
              sentenceStarts.has(index) ? sentence.speaker : undefined,
            )
          : undefined,
      animateNewest: this.animateNewestSentence,
      lines: sentenceMode || this.speakerLabelsMode ? "multi" : "single",
      state: sentenceMode || this.activeFinal ? "final" : "partial",
      opacity: 1,
    };
    this.animateNewestSentence = false;
    this.onChange(this.lastFrame);
  }

  clear(immediate = false): void {
    this.activeSegmentId = 0;
    this.activeRevision = -1;
    this.activeFinal = false;
    this.finalizedLiveWords = [];
    this.activeLiveSegment = undefined;
    this.sentences = [];
    this.animateNewestSentence = false;
    this.current = undefined;
    this.render(immediate);
  }

  dispose(): void {
    this.clear(true);
    this.measurer?.dispose();
  }
}

/**
 * Agrupa as palavras visíveis em trechos: um novo trecho a cada rótulo de
 * falante e na fronteira entre texto confirmado e palavras recentes.
 */
function partsOf(words: readonly LiveWord[], splitAt: number): CaptionPart[] {
  const starts = labelStarts(words.map((word) => word.speaker));
  const parts: CaptionPart[] = [];
  words.forEach((word, index) => {
    const partial = index >= splitAt;
    const last = parts.at(-1);
    if (last && !starts.has(index) && last.partial === partial) {
      last.text += ` ${word.text}`;
      return;
    }
    parts.push({
      text: word.text,
      partial,
      ...(starts.has(index) ? { speaker: word.speaker } : {}),
    });
  });
  return parts;
}
