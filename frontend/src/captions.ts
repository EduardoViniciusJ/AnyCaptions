import type { CaptionMode, CaptionUpdate } from "./contracts";

export interface CaptionClock {
  set(callback: () => void, delay: number): ReturnType<typeof setTimeout>;
  clear(handle: ReturnType<typeof setTimeout>): void;
}

const browserClock: CaptionClock = {
  set: (callback, delay) => setTimeout(callback, delay),
  clear: (handle) => clearTimeout(handle),
};

// Live captions roll up like Windows Live Captions: a fixed box shows the last lines of the
// transcript; when the bottom line fills, the text moves up one line.
export const LIVE_LINES = 1;
// The text stays through pauses so the next phrase keeps writing after it; after this much
// silence it fades out and the next speech starts on an empty box.
export const LIVE_IDLE_CLEAR_MS = 3000;
// Long-running speech keeps only recent words; older ones are already off screen.
const LIVE_TRANSCRIPT_COMPACT_WORDS = 120;
// Visible fade when a caption leaves the screen; matches the CSS opacity transition.
export const CAPTION_FADE_MS = 300;

export function readingTime(text: string): number {
  return Math.max(2000, Math.min(7000, Math.ceil((text.length / 15) * 1000)));
}

/**
 * Greedy word wrap. Wrapping a longer transcript never changes the earlier lines, so the
 * visible text only grows at the end or moves up a whole line.
 */
export function wrapLines(words: string[], charsPerLine: number): string[][] {
  const lines: string[][] = [];
  let line: string[] = [];
  let length = 0;
  for (const word of words) {
    if (line.length && length + 1 + word.length > charsPerLine) {
      lines.push(line);
      line = [];
      length = 0;
    }
    length += (line.length ? 1 : 0) + word.length;
    line.push(word);
  }
  if (line.length) lines.push(line);
  return lines;
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
  segmentId: number;
  // Live roll-up only: visible lines, transcript index of their first word, and the first
  // transcript index added by the latest update (those words fade in).
  lines?: string[][];
  firstIndex?: number;
  newFrom?: number;
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
  // Live words: finished phrases followed by the latest revision of the active one.
  private transcript: string[] = [];
  private activeStart = 0; // transcript index where the active phrase begins
  private firstVisible = 0; // transcript index of the first word on the top visible line
  private current: Block | undefined;
  private queue: Block[] = [];
  private timer: ReturnType<typeof setTimeout> | undefined;
  private fadeTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly element: HTMLElement,
    private readonly clock: CaptionClock = browserClock,
    private readonly onChange: () => void = () => {},
  ) {
    // The live box height follows LIVE_LINES (see #caption-live in styles.css).
    element.style.setProperty("--caption-lines", String(LIVE_LINES));
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

  private acceptLive(update: CaptionUpdate): boolean {
    if (
      update.segmentId < this.activeSegmentId ||
      (update.segmentId === this.activeSegmentId && (this.activeFinal || update.revision <= this.activeRevision))
    )
      return false;

    const words = update.text.trim().split(/\s+/u).filter(Boolean);
    if (update.segmentId !== this.activeSegmentId) this.activeStart = this.transcript.length;
    const previousLength = this.transcript.length;
    // Every revision is shown at once; the next one replaces the words it corrects.
    this.transcript.splice(this.activeStart, previousLength - this.activeStart, ...words);
    this.activeSegmentId = update.segmentId;
    this.activeRevision = update.revision;
    this.activeFinal = update.isFinal;

    this.firstVisible = Math.min(this.firstVisible, this.transcript.length);
    let lines = wrapLines(this.transcript.slice(this.firstVisible), this.charsPerLine());
    if (!lines.length) return true;
    if (lines.length > LIVE_LINES) {
      // Roll up: whole lines leave the top, so the remaining text keeps its position.
      for (const line of lines.slice(0, lines.length - LIVE_LINES)) this.firstVisible += line.length;
      lines = lines.slice(-LIVE_LINES);
    }
    const removed = this.compactTranscript();

    this.cancelTimer();
    this.current = {
      text: lines.map((line) => line.join(" ")).join("\n"),
      segmentId: update.segmentId,
      lines,
      firstIndex: this.firstVisible,
      // Words appended by this update fade in; corrections of shown words swap in place.
      newFrom: previousLength - removed,
    };
    this.render(false);
    this.timer = this.clock.set(() => {
      this.timer = undefined;
      this.resetTranscript();
      this.current = undefined;
      this.render(false);
    }, LIVE_IDLE_CLEAR_MS);
    return true;
  }

  /** Drops off-screen words; returns how many indices shifted. */
  private compactTranscript(): number {
    // The active phrase must stay addressable, since a final may still replace it.
    const removable = Math.min(this.firstVisible, this.activeStart);
    if (removable < LIVE_TRANSCRIPT_COMPACT_WORDS) return 0;
    this.transcript.splice(0, removable);
    this.firstVisible -= removable;
    this.activeStart -= removable;
    return removable;
  }

  private resetTranscript(): void {
    this.transcript = [];
    this.activeStart = 0;
    this.firstVisible = 0;
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
      }, CAPTION_FADE_MS);
      return;
    }
    if (this.fadeTimer !== undefined) this.clock.clear(this.fadeTimer);
    this.fadeTimer = undefined;
    // Partials stay immediate, including arrivals during an outgoing fade.
    this.element.style.transition = "none";
    this.element.style.opacity = animate && !reducedMotion ? "0" : "1";

    if (this.current?.lines) {
      const nodes: Node[] = [];
      let index = this.current.firstIndex ?? 0;
      const newFrom = this.current.newFrom ?? Number.POSITIVE_INFINITY;
      this.current.lines.forEach((line, lineIndex) => {
        if (lineIndex) nodes.push(document.createTextNode("\n"));
        line.forEach((word, wordIndex) => {
          if (wordIndex) nodes.push(document.createTextNode(" "));
          const span = document.createElement("span");
          span.textContent = word;
          if (index >= newFrom) span.className = "caption-word-new";
          nodes.push(span);
          index++;
        });
      });
      this.element.replaceChildren(...nodes);
    } else {
      this.element.textContent = this.current?.text ?? "";
    }

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
    this.resetTranscript();
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
