import { ptBR } from "./pt-br";
import { en } from "./en";
import { es } from "./es";
import { fr } from "./fr";
import { ru } from "./ru";
import { zhCN } from "./zh-cn";

export type MessageKey = keyof typeof ptBR;
export type Messages = Record<MessageKey, string>;

/**
 * Idiomas da interface, espelhando `UI_LANGUAGES` do backend (que valida a
 * escolha). Os nomes são endônimos, então não precisam de tradução.
 */
export interface UiLanguageOption {
  tag: string;
  /** Código do país da bandeira (`assets/flags/<código>.svg`). */
  country: string;
  name: string;
}

export const UI_LANGUAGES: UiLanguageOption[] = [
  { tag: "pt-br", country: "br", name: "Português (Brasil)" },
  { tag: "en", country: "us", name: "English" },
  { tag: "es", country: "es", name: "Español" },
  { tag: "zh-cn", country: "cn", name: "中文（简体）" },
  { tag: "fr", country: "fr", name: "Français" },
  { tag: "ru", country: "ru", name: "Русский" },
];

const locales: Record<string, Messages> = {
  "pt-br": ptBR,
  en,
  es,
  "zh-cn": zhCN,
  fr,
  ru,
};

let currentTag = "pt-br";
let current: Messages = ptBR;

/** Troca o idioma atual; devolve `true` quando ele mudou. */
export function setLocale(tag: string): boolean {
  const next = locales[tag];
  if (!next || tag === currentTag) return false;
  currentTag = tag;
  current = next;
  return true;
}

export function localeTag(): string {
  return currentTag;
}

export function t(key: MessageKey): string {
  return current[key];
}
