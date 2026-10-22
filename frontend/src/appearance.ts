import type { Appearance, CaptionColor } from "./contracts";

export const MIN_CAPTION_SIZE = 16;
export const MAX_CAPTION_SIZE = 80;
export const DEFAULT_APPEARANCE: Appearance = {
  fontSize: 32,
  color: "#FFFFFF",
  partialColor: "#B8B8B8",
};

const HEX_COLOR = /^#[0-9A-F]{6}$/u;

export function normalizeCaptionColor(value: string): CaptionColor | undefined {
  const normalized = value.trim().toUpperCase();
  return HEX_COLOR.test(normalized) ? normalized as CaptionColor : undefined;
}

export function sameAppearance(left: Appearance, right: Appearance): boolean {
  return left.fontSize === right.fontSize
    && left.color === right.color
    && left.partialColor === right.partialColor;
}

export function applyAppearance(element: HTMLElement, appearance: Appearance): void {
  element.style.setProperty("--caption-base-size", `${appearance.fontSize}px`);
  element.style.setProperty("--caption-color", appearance.color);
  element.style.setProperty("--caption-partial-color", appearance.partialColor);
}
