import type { CaptionMode, CaptionUpdate } from "./contracts";

export interface CaptionClock {
  set(callback: () => void, delay: number): ReturnType<typeof setTimeout>;
  clear(handle: ReturnType<typeof setTimeout>): void;
}

const browserClock: CaptionClock = { set: (callback, delay) => setTimeout(callback, delay), clear: (handle) => clearTimeout(handle) };

export function readingTime(text: string): number {
  return Math.max(2000, Math.min(7000, Math.ceil(text.length / 15 * 1000)));
}

// Each block is at most three word-wrapped lines; long translations continue in later blocks.
export function splitCaption(text: string, charsPerLine = 42, maxLines = 3): string[] {
  const words = text.trim().split(/\s+/u).filter(Boolean);
  const blocks: string[] = [];
  let lines: string[] = [];
  let line = "";
  for (const word of words) {
    if (line && line.length + 1 + word.length > charsPerLine) {
      lines.push(line);
      line = "";
      if (lines.length === maxLines) { blocks.push(lines.join("\n")); lines = []; }
    }
    line = line ? `${line} ${word}` : word;
  }
  if (line) lines.push(line);
  if (lines.length) blocks.push(lines.join("\n"));
  return blocks;
}

interface Block { text: string; segmentId: number }

export class CaptionPresenter {
  private sessionId = 0;
  private mode: CaptionMode = "en-en";
  private revisions = new Map<number, number>();
  private finished = new Set<number>();
  private pending = new Map<number, CaptionUpdate>();
  private nextSegmentId = 1;
  private activeSegmentId = 0;
  private partial = "";
  private current: Block | undefined;
  private previousStable = "";
  private queue: Block[] = [];
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly element: HTMLElement, private readonly clock: CaptionClock = browserClock,
    private readonly onChange: () => void = () => {}) {}

  setContext(sessionId: number, mode: CaptionMode): void {
    if (sessionId === this.sessionId && mode === this.mode) return;
    this.clear();
    this.sessionId = sessionId;
    this.mode = mode;
  }

  accept(update: CaptionUpdate): boolean {
    if (update.sessionId !== this.sessionId || update.mode !== this.mode || update.segmentId < 1) return false;
    if (!Number.isSafeInteger(update.revision) || update.revision < 0 || update.text.length > 4000) return false;
    const prior = this.revisions.get(update.segmentId) ?? -1;
    if (update.revision <= prior || this.finished.has(update.segmentId) || update.segmentId < this.nextSegmentId) return false;
    if (this.mode === "en-pt" && !update.isFinal) return false;
    const blocks = update.isFinal ? splitCaption(update.text, this.charsPerLine(), this.maxLines()) : [];
    const pendingBlocks = [...this.pending.values()].reduce((sum, item) =>
      sum + splitCaption(item.text, this.charsPerLine(), this.maxLines()).length, 0);
    if (this.queue.length + pendingBlocks + blocks.length > 32 ||
      (this.revisions.size >= 64 && !this.revisions.has(update.segmentId))) return false;
    this.revisions.set(update.segmentId, update.revision);
    if (!update.isFinal) {
      if (update.segmentId >= this.activeSegmentId) {
        this.activeSegmentId = update.segmentId;
        this.partial = update.text.trim();
        this.render();
      }
      return true;
    }
    this.finished.add(update.segmentId);
    if (update.segmentId === this.activeSegmentId) this.partial = "";
    this.pending.set(update.segmentId, update);
    this.flushOrdered();
    this.render();
    return true;
  }

  private flushOrdered(): void {
    while (this.pending.has(this.nextSegmentId)) {
      const update = this.pending.get(this.nextSegmentId)!;
      this.pending.delete(this.nextSegmentId);
      const blocks = splitCaption(update.text, this.charsPerLine(), this.maxLines())
        .map((text) => ({ text, segmentId: update.segmentId }));
      if (this.mode === "en-en") {
        this.previousStable = this.current?.text.split("\n").length === 1 ? this.current.text : "";
        this.queue = blocks;
        this.advance(true);
      } else {
        this.queue.push(...blocks);
      }
      this.revisions.delete(this.nextSegmentId);
      this.finished.delete(this.nextSegmentId);
      this.nextSegmentId++;
    }
    if (!this.current && this.queue.length) this.advance();
  }

  private charsPerLine(): number {
    const width = this.element.clientWidth;
    const fontSize = Number.parseFloat(this.element.style.getPropertyValue("--caption-base-size")) || 32;
    return width > 0 ? Math.max(10, Math.floor((width - 32) / (fontSize * 0.7))) : 42;
  }

  private maxLines(): number {
    const height = this.element.parentElement?.clientHeight ?? 0;
    if (!height) return 3;
    const fontSize = Number.parseFloat(this.element.style.getPropertyValue("--caption-base-size")) || 32;
    return Math.max(1, Math.min(3, Math.floor((height - 16) / (fontSize * 1.25))));
  }

  private advance(preservePrevious = false): void {
    if (this.timer !== undefined) this.clock.clear(this.timer);
    if (!preservePrevious) this.previousStable = "";
    this.current = this.queue.shift();
    this.render();
    if (this.current) {
      this.timer = this.clock.set(() => { this.timer = undefined; this.advance(); }, readingTime(this.current.text));
    }
  }

  private render(): void {
    const current = this.current?.text ?? "";
    const priorLines = this.previousStable ? 1 : 0;
    const currentLines = current ? current.split("\n").length : 0;
    const stable = priorLines + currentLines <= this.maxLines()
      ? [this.previousStable, current].filter(Boolean).join("\n") : current;
    const stableLines = stable ? stable.split("\n").length : 0;
    const partial = this.mode === "en-en" && stableLines < this.maxLines() ? this.partial : "";
    this.element.textContent = [stable, partial].filter(Boolean).join("\n");
    this.element.dataset.state = partial ? "partial" : "final";
    this.element.hidden = !this.element.textContent;
    this.onChange();
  }

  clear(): void {
    if (this.timer !== undefined) this.clock.clear(this.timer);
    this.timer = undefined;
    this.revisions.clear();
    this.finished.clear();
    this.pending.clear();
    this.nextSegmentId = 1;
    this.activeSegmentId = 0;
    this.partial = "";
    this.current = undefined;
    this.previousStable = "";
    this.queue = [];
    this.render();
  }

  dispose(): void { this.clear(); }
}
