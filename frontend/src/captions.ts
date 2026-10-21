import type { CaptionMode, CaptionUpdate } from "./contracts";

export interface CaptionClock {
  set(callback: () => void, delay: number): ReturnType<typeof setTimeout>;
  clear(handle: ReturnType<typeof setTimeout>): void;
}

const browserClock: CaptionClock = {
  set: (callback, delay) => setTimeout(callback, delay),
  clear: (handle) => clearTimeout(handle),
};

export const LIVE_PHRASE_MAX_WORDS = 9;
export const LIVE_PHRASE_MAX_CHARS = 64;
const MIN_ROLLING_WINDOW_WORDS = 5;

export function readingTime(text: string): number {
  return Math.max(2000, Math.min(7000, Math.ceil((text.length / 15) * 1000)));
}

export function liveCaptionTime(text: string): number {
  return Math.max(2000, Math.min(5000, Math.ceil((text.length / 18) * 1000)));
}

export function splitTrailingWord(text: string): { leading: string; trailing: string } {
  const words = text.trim().split(/\s+/u).filter(Boolean);
  if (words.length === 0) return { leading: "", trailing: "" };
  if (words.length === 1) return { leading: "", trailing: words[0]! };
  return {
    leading: words.slice(0, -1).join(" "),
    trailing: words.at(-1)!,
  };
}

export function splitPhraseChunks(
  text: string,
  maxChars = LIVE_PHRASE_MAX_CHARS,
  maxWords = LIVE_PHRASE_MAX_WORDS,
): string[] {
  const words = text.trim().split(/\s+/u).filter(Boolean);
  const chunks: string[] = [];
  let current: string[] = [];
  let currentLen = 0;
  for (const word of words) {
    const nextLen = currentLen ? currentLen + 1 + word.length : word.length;
    if (current.length > 0 && (nextLen > maxChars || current.length >= maxWords)) {
      chunks.push(current.join(" "));
      current = [word];
      currentLen = word.length;
    } else {
      current.push(word);
      currentLen = nextLen;
    }
  }
  if (current.length > 0) {
    // Prevent 1-2 word orphans (like a lone "the") when a phrase rolls over:
    // borrow trailing words from the previous chunk so the visible line always has context.
    if (chunks.length > 0 && current.length < 4 && words.length >= MIN_ROLLING_WINDOW_WORDS) {
      const windowWords = words.slice(-MIN_ROLLING_WINDOW_WORDS);
      chunks.push(windowWords.join(" "));
    } else {
      chunks.push(current.join(" "));
    }
  }
  return chunks;
}

// Long translations keep every word in ordered blocks; live English follows the newest cue.
export function splitCaption(text: string, charsPerLine = 42, maxLines = 2): string[] {
  const words = text.trim().split(/\s+/u).filter(Boolean);
  const blocks: string[] = [];
  let lines: string[] = [];
  let line = "";
  for (const word of words) {
    if (line && line.length + 1 + word.length > charsPerLine) {
      lines.push(line);
      line = "";
      if (lines.length === maxLines) {
        blocks.push(lines.join("\n"));
        lines = [];
      }
    }
    line = line ? `${line} ${word}` : word;
  }
  if (line) lines.push(line);
  if (lines.length) blocks.push(lines.join("\n"));
  return blocks;
}

interface Block {
  text: string;
  committedText?: string;
  livePartialText?: string;
  segmentId: number;
}

export class CaptionPresenter {
  private sessionId = 0;
  private mode: CaptionMode = "en-en";
  private revisions = new Map<number, number>();
  private finished = new Set<number>();
  private pending = new Map<number, CaptionUpdate>();
  private nextSegmentId = 1;
  private activeSegmentId = 0;
  private activeRevision = -1;
  private activeFinal = false;
  private current: Block | undefined;
  private queue: Block[] = [];
  private timer: ReturnType<typeof setTimeout> | undefined;
  private fadeTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly element: HTMLElement,
    private readonly clock: CaptionClock = browserClock,
    private readonly onChange: () => void = () => {},
  ) {}

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
    if (this.mode === "en-en") return this.acceptLive(update);
    const prior = this.revisions.get(update.segmentId) ?? -1;
    if (update.revision <= prior || this.finished.has(update.segmentId) || update.segmentId < this.nextSegmentId)
      return false;
    if (this.mode === "en-pt" && !update.isFinal) return false;
    const blocks = update.isFinal ? splitCaption(update.text, this.charsPerLine(), this.maxLines()) : [];
    const pendingBlocks = [...this.pending.values()].reduce(
      (sum, item) => sum + splitCaption(item.text, this.charsPerLine(), this.maxLines()).length,
      0,
    );
    if (
      this.queue.length + pendingBlocks + blocks.length > 32 ||
      (this.revisions.size >= 64 && !this.revisions.has(update.segmentId))
    )
      return false;
    this.revisions.set(update.segmentId, update.revision);
    this.finished.add(update.segmentId);
    this.pending.set(update.segmentId, update);
    this.flushOrdered();
    return true;
  }

  private phraseCharLimit(): number {
    const width = this.element.clientWidth || this.element.parentElement?.clientWidth || 0;
    if (!width) return LIVE_PHRASE_MAX_CHARS;
    return Math.max(52, Math.min(LIVE_PHRASE_MAX_CHARS, this.charsPerLine() * this.maxLines()));
  }

  private acceptLive(update: CaptionUpdate): boolean {
    if (
      update.segmentId < this.activeSegmentId ||
      (update.segmentId === this.activeSegmentId && (this.activeFinal || update.revision <= this.activeRevision))
    )
      return false;

    this.activeSegmentId = update.segmentId;
    this.activeRevision = update.revision;
    this.activeFinal = update.isFinal;

    const maxChars = this.phraseCharLimit();
    const chunks = splitPhraseChunks(update.text, maxChars, LIVE_PHRASE_MAX_WORDS);
    const phrase = chunks.at(-1) ?? "";
    if (!phrase) return true;

    let committedText = "";
    let livePartialText = "";
    if (update.isFinal) {
      committedText = phrase;
      livePartialText = "";
    } else {
      const { leading, trailing } = splitTrailingWord(phrase);
      committedText = leading;
      livePartialText = trailing;
    }

    this.cancelTimer();
    this.current = {
      text: phrase,
      committedText,
      livePartialText,
      segmentId: update.segmentId,
    };
    this.render(false);
    return true;
  }

  private flushOrdered(): void {
    while (this.pending.has(this.nextSegmentId)) {
      const update = this.pending.get(this.nextSegmentId)!;
      this.pending.delete(this.nextSegmentId);
      const blocks = splitCaption(update.text, this.charsPerLine(), this.maxLines()).map((text) => ({
        text,
        segmentId: update.segmentId,
      }));
      this.queue.push(...blocks);
      this.revisions.delete(this.nextSegmentId);
      this.finished.delete(this.nextSegmentId);
      this.nextSegmentId++;
    }
    if ((!this.current && this.queue.length) || (this.current && this.queue.length && this.timer === undefined)) {
      this.advance();
    }
  }

  private charsPerLine(): number {
    const width = this.element.clientWidth || this.element.parentElement?.clientWidth || 0;
    const fontSize =
      Number.parseFloat(getComputedStyle(this.element).fontSize) ||
      Number.parseFloat(this.element.style.getPropertyValue("--caption-base-size")) ||
      32;
    return width > 0 ? Math.max(10, Math.min(42, Math.floor((width - 32) / (fontSize * 0.7)))) : 42;
  }

  private maxLines(): number {
    const height = this.element.parentElement?.clientHeight ?? 0;
    if (!height) return 2;
    const fontSize = Number.parseFloat(this.element.style.getPropertyValue("--caption-base-size")) || 32;
    return Math.max(1, Math.min(2, Math.floor((height - 16) / (fontSize * 1.25))));
  }

  private advance(): void {
    this.cancelTimer();
    this.current = this.queue.shift();
    this.render(true);
    if (this.current && this.queue.length > 0) {
      this.timer = this.clock.set(() => {
        this.timer = undefined;
        this.advance();
      }, readingTime(this.current.text));
    }
  }

  private render(animate = false, immediate = false): void {
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    if (!this.current && !immediate && !reducedMotion && this.element.textContent) {
      if (this.fadeTimer !== undefined) return;
      this.element.style.opacity = "0";
      this.fadeTimer = this.clock.set(() => {
        this.fadeTimer = undefined;
        this.render(false, true);
      }, 120);
      return;
    }
    if (this.fadeTimer !== undefined) this.clock.clear(this.fadeTimer);
    this.fadeTimer = undefined;
    // Partials stay immediate, including arrivals during an outgoing fade.
    this.element.style.transition = "none";
    this.element.style.opacity = animate && !reducedMotion ? "0" : "1";

    if (this.current && this.current.livePartialText) {
      const committedSpan = document.createElement("span");
      committedSpan.className = "caption-final";
      committedSpan.textContent = this.current.committedText ? `${this.current.committedText} ` : "";

      const partialSpan = document.createElement("span");
      partialSpan.className = "caption-partial";
      partialSpan.textContent = this.current.livePartialText;

      if (committedSpan.textContent) {
        this.element.replaceChildren(committedSpan, partialSpan);
      } else {
        this.element.replaceChildren(partialSpan);
      }
    } else {
      this.element.textContent = this.current?.text ?? "";
    }

    this.element.dataset.state = this.mode === "en-en" && !this.activeFinal ? "partial" : "final";
    this.element.hidden = !this.element.textContent;
    // Commit the starting opacity before enabling the CSS transition.
    void this.element.offsetWidth;
    this.element.style.transition = "";
    this.element.style.opacity = "1";
    this.onChange();
  }

  clear(immediate = false): void {
    this.cancelTimer();
    this.revisions.clear();
    this.finished.clear();
    this.pending.clear();
    this.nextSegmentId = 1;
    this.activeSegmentId = 0;
    this.activeRevision = -1;
    this.activeFinal = false;
    this.current = undefined;
    this.queue = [];
    this.render(false, immediate);
  }

  private cancelTimer(): void {
    if (this.timer !== undefined) this.clock.clear(this.timer);
    this.timer = undefined;
  }

  dispose(): void {
    this.clear(true);
  }
}
