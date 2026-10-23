/** Cabe no espaço visível? `maxLines` linhas (1 por padrão) na largura atual. */
export type CaptionFits = (text: string, maxLines?: number) => boolean;

const FALLBACK_CAPTION_CHARS = 84;

export function captionWords(text: string): string[] {
  return text.trim().split(/\s+/u).filter(Boolean);
}

export function paginateCaption(text: string, fits: CaptionFits): string[] {
  const words = captionWords(text);
  const pages: string[] = [];
  let start = 0;

  while (start < words.length) {
    let lower = start + 1;
    let upper = words.length;
    let end = lower;

    while (lower <= upper) {
      const middle = Math.floor((lower + upper) / 2);
      const candidate = words.slice(start, middle).join(" ");
      if (fits(candidate)) {
        end = middle;
        lower = middle + 1;
      } else {
        upper = middle - 1;
      }
    }

    pages.push(words.slice(start, end).join(" "));
    start = end;
  }

  return pages;
}

export class CaptionMeasurer {
  private readonly probe: HTMLElement;

  constructor(private readonly source: HTMLElement) {
    this.probe = source.cloneNode(false) as HTMLElement;
    this.probe.removeAttribute("id");
    this.probe.removeAttribute("aria-live");
    this.probe.setAttribute("aria-hidden", "true");
    this.probe.hidden = false;
    this.probe.style.position = "fixed";
    this.probe.style.inset = "0 auto auto -10000px";
    this.probe.style.visibility = "hidden";
    this.probe.style.pointerEvents = "none";
    this.probe.style.height = "auto";
    this.probe.style.maxHeight = "none";
    this.probe.style.overflow = "visible";
    this.probe.style.transition = "none";
    document.body.append(this.probe);
  }

  fits = (text: string, maxLines = 1): boolean => {
    const width =
      this.source.clientWidth ||
      this.source.parentElement?.clientWidth ||
      this.source.closest<HTMLElement>(".overlay")?.clientWidth ||
      0;
    if (width <= 0) return text.length <= FALLBACK_CAPTION_CHARS;

    const sourceStyle = getComputedStyle(this.source);
    this.probe.className = this.source.className;
    this.probe.style.width = `${width}px`;
    this.probe.style.padding = sourceStyle.padding;
    this.probe.style.font = sourceStyle.font;
    this.probe.style.fontSize = sourceStyle.fontSize;
    this.probe.style.fontWeight = sourceStyle.fontWeight;
    this.probe.style.letterSpacing = sourceStyle.letterSpacing;
    this.probe.style.lineHeight = sourceStyle.lineHeight;
    this.probe.style.whiteSpace = "normal";
    this.probe.style.setProperty(
      "--caption-base-size",
      this.source.style.getPropertyValue("--caption-base-size"),
    );
    this.probe.textContent = text;

    const fontSize = Number.parseFloat(sourceStyle.fontSize) || 32;
    const lineHeight = Number.parseFloat(sourceStyle.lineHeight) || fontSize * 1.25;
    const padding =
      (Number.parseFloat(sourceStyle.paddingTop) || 0) +
      (Number.parseFloat(sourceStyle.paddingBottom) || 0);
    return this.probe.scrollHeight <= lineHeight * maxLines + padding + 1;
  };

  dispose(): void {
    this.probe.remove();
  }
}
