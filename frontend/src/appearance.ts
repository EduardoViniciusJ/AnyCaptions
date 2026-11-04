import type { Appearance, CaptionColor, CaptionFont } from "./contracts";

export const MIN_CAPTION_SIZE = 16;
export const MAX_CAPTION_SIZE = 80;
export const DEFAULT_APPEARANCE: Appearance = {
  fontSize: 32,
  color: "#FFFFFF",
  partialColor: "#B8B8B8",
  background: false,
  font: "inter",
};

/**
 * Fontes das legendas, espelhando `CAPTION_FONTS` do backend (que valida o
 * id). Inter e Fira Code vêm empacotadas com o app; as demais existem em
 * qualquer Windows. `family` é a pilha CSS; o nome é mostrado na própria fonte.
 */
export interface CaptionFontOption {
  id: CaptionFont;
  name: string;
  family: string;
}

export const CAPTION_FONTS: CaptionFontOption[] = [
  { id: "inter", name: "Inter", family: '"Inter Variable", "Segoe UI", Arial, sans-serif' },
  { id: "fira-code", name: "Fira Code", family: '"Fira Code Variable", Consolas, monospace' },
  { id: "segoe-ui", name: "Segoe UI", family: '"Segoe UI", Arial, sans-serif' },
  { id: "arial", name: "Arial", family: "Arial, Helvetica, sans-serif" },
  { id: "verdana", name: "Verdana", family: "Verdana, Geneva, sans-serif" },
  { id: "tahoma", name: "Tahoma", family: "Tahoma, Geneva, sans-serif" },
  { id: "trebuchet-ms", name: "Trebuchet MS", family: '"Trebuchet MS", Arial, sans-serif' },
  { id: "georgia", name: "Georgia", family: "Georgia, 'Times New Roman', serif" },
  { id: "consolas", name: "Consolas", family: "Consolas, 'Courier New', monospace" },
];

/** Pilha CSS da fonte escolhida; ids desconhecidos caem na padrão (Inter). */
export function captionFontFamily(id: CaptionFont): string {
  return (CAPTION_FONTS.find((font) => font.id === id) ?? CAPTION_FONTS[0]!).family;
}

const HEX_COLOR = /^#[0-9A-F]{6}$/u;

export function normalizeCaptionColor(value: string): CaptionColor | undefined {
  const normalized = value.trim().toUpperCase();
  return HEX_COLOR.test(normalized) ? normalized as CaptionColor : undefined;
}

export function sameAppearance(left: Appearance, right: Appearance): boolean {
  return left.fontSize === right.fontSize
    && left.color === right.color
    && left.partialColor === right.partialColor
    && left.background === right.background
    && left.font === right.font;
}

export function applyAppearance(element: HTMLElement, appearance: Appearance): void {
  element.style.setProperty("--caption-base-size", `${appearance.fontSize}px`);
  element.style.setProperty("--caption-color", appearance.color);
  element.style.setProperty("--caption-partial-color", appearance.partialColor);
  element.style.setProperty("--caption-font", captionFontFamily(appearance.font));
  element.toggleAttribute("data-background", appearance.background);
}
