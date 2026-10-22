import type { CaptionMode, CaptionUpdate } from "./contracts";
import {
  CaptionMeasurer,
  captionWords,
  paginateCaption,
  type CaptionFits,
} from "./caption-layout";

export interface CaptionClock {
  set(callback: () => void, delay: number): ReturnType<typeof setTimeout>;
  clear(handle: ReturnType<typeof setTimeout>): void;
}

const browserClock: CaptionClock = {
  set: (callback, delay) => setTimeout(callback, delay),
  clear: (handle) => clearTimeout(handle),
};

const CLEAR_FADE_MS = 120;

export function splitTrailingWords(text: string, trailingWords = 1): { leading: string; trailing: string } {
  const words = text.trim().split(/\s+/u).filter(Boolean);
  if (words.length === 0) return { leading: "", trailing: "" };
  const splitAt = Math.max(0, words.length - trailingWords);
  return {
    leading: words.slice(0, splitAt).join(" "),
    trailing: words.slice(splitAt).join(" "),
  };
}

interface Block {
  text: string;
  committedText?: string;
  livePartialText?: string;
  segmentId: number;
}

interface ActiveLiveSegment {
  words: string[];
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
  private activeSegmentId = 0;
  private activeRevision = -1;
  private activeFinal = false;
  private finalizedLiveWords: string[] = [];
  private activeLiveSegment: ActiveLiveSegment | undefined;
  private current: Block | undefined;
  private fadeTimer: ReturnType<typeof setTimeout> | undefined;
  private readonly measurer: CaptionMeasurer | undefined;
  private readonly fitsCaption: CaptionFits;

  constructor(
    private readonly element: HTMLElement,
    private readonly clock: CaptionClock = browserClock,
    private readonly onChange: () => void = () => {},
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

  accept(update: CaptionUpdate): boolean {
    if (
      update.sessionId !== this.sessionId ||
      update.mode !== this.mode ||
      !Number.isSafeInteger(update.segmentId) ||
      update.segmentId < 1
    )
      return false;
    if (!Number.isSafeInteger(update.revision) || update.revision < 0 || update.text.length > 4000) return false;
    if (
      update.segmentId < this.activeSegmentId ||
      (update.segmentId === this.activeSegmentId && (this.activeFinal || update.revision <= this.activeRevision))
    )
      return false;

    const newSegment = update.segmentId !== this.activeSegmentId;
    if (newSegment) this.commitActiveLiveSegment();

    this.activeSegmentId = update.segmentId;
    this.activeRevision = update.revision;
    this.activeFinal = update.isFinal;

    const incomingWords = captionWords(update.text);
    const previous = this.activeLiveSegment;
    const previousWords = previous?.words ?? [];
    const regressivePartial =
      !update.isFinal &&
      incomingWords.length < previousWords.length &&
      incomingWords.every((word, index) => word === previousWords[index]);

    this.activeLiveSegment = {
      words: regressivePartial ? previousWords : incomingWords,
      consumedWords: newSegment ? 0 : (previous?.consumedWords ?? 0),
    };

    this.layoutLiveCaption();
    if (update.isFinal) this.commitActiveLiveSegment();
    this.render();
    return true;
  }

  private liveWords(): string[] {
    const activeWords = this.activeLiveSegment
      ? this.activeLiveSegment.words.slice(this.activeLiveSegment.consumedWords)
      : [];
    return [...this.finalizedLiveWords, ...activeWords];
  }

  private commitActiveLiveSegment(): void {
    if (!this.activeLiveSegment) return;
    this.finalizedLiveWords.push(
      ...this.activeLiveSegment.words.slice(this.activeLiveSegment.consumedWords),
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
    const pages = paginateCaption(this.liveWords().join(" "), this.fitsCaption);
    const visible = pages.at(-1) ?? "";
    const discardedWords = pages
      .slice(0, -1)
      .reduce((total, page) => total + captionWords(page).length, 0);
    if (discardedWords > 0) this.consumeLiveWords(discardedWords);

    const activeVisibleWords = this.activeLiveSegment
      ? this.activeLiveSegment.words.length - this.activeLiveSegment.consumedWords
      : 0;
    const { leading, trailing } = this.activeFinal
      ? { leading: visible, trailing: "" }
      : splitTrailingWords(visible, Math.min(2, activeVisibleWords));
    this.current = visible
      ? {
          text: visible,
          committedText: leading,
          livePartialText: trailing,
          segmentId: this.activeSegmentId,
        }
      : undefined;
  }

  refreshLayout(): void {
    if (this.liveWords().length === 0) return;
    this.layoutLiveCaption();
    this.render();
  }

  private render(immediate = false): void {
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    if (!this.current && !immediate && !reducedMotion && this.element.textContent) {
      if (this.fadeTimer !== undefined) return;
      this.element.style.opacity = "0";
      this.fadeTimer = this.clock.set(() => {
        this.fadeTimer = undefined;
        this.render(true);
      }, CLEAR_FADE_MS);
      return;
    }
    if (this.fadeTimer !== undefined) this.clock.clear(this.fadeTimer);
    this.fadeTimer = undefined;
    this.element.style.transition = "none";
    this.element.style.opacity = "1";

    if (!this.current) {
      this.element.replaceChildren();
      this.element.hidden = true;
      void this.element.offsetWidth;
      this.element.style.transition = "";
      this.onChange();
      return;
    }

    const content = document.createElement("span");
    content.className = "caption-content";

    if (this.current.livePartialText) {
      const committedSpan = document.createElement("span");
      committedSpan.className = "caption-final";
      committedSpan.textContent = this.current.committedText ? `${this.current.committedText} ` : "";

      const partialSpan = document.createElement("span");
      partialSpan.className = "caption-partial";
      partialSpan.textContent = this.current.livePartialText;

      if (committedSpan.textContent) {
        content.replaceChildren(committedSpan, partialSpan);
      } else {
        content.replaceChildren(partialSpan);
      }
    } else {
      content.textContent = this.current.text;
    }
    this.element.replaceChildren(content);

    this.element.dataset.state = !this.activeFinal ? "partial" : "final";
    this.element.hidden = false;
    void this.element.offsetWidth;
    this.element.style.transition = "";

    this.onChange();
  }

  clear(immediate = false): void {
    this.activeSegmentId = 0;
    this.activeRevision = -1;
    this.activeFinal = false;
    this.finalizedLiveWords = [];
    this.activeLiveSegment = undefined;
    this.current = undefined;
    this.render(immediate);
  }

  dispose(): void {
    this.clear(true);
    this.measurer?.dispose();
  }
}
