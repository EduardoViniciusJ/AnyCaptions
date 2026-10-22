import type { CaptionMode } from "./contracts";

/**
 * Idiomas-alvo de tradução, espelhando `TRANSLATION_TARGETS` do backend
 * (a lista oficial do modelo). O backend valida o valor; esta tabela só
 * apresenta. Ordem: PT-BR (padrão), EN, demais por nome.
 */
export interface TranslationTarget {
  mode: CaptionMode;
  /** Sigla curta mostrada no rótulo (variantes se distinguem pela bandeira). */
  short: string;
  flag: string;
  name: string;
}

export const TRANSLATION_TARGETS: TranslationTarget[] = [
  { mode: "translate-pt-br", short: "PT", flag: "🇧🇷", name: "Português (Brasil)" },
  { mode: "translate-en", short: "EN", flag: "🇺🇸", name: "Inglês" },
  { mode: "translate-de", short: "DE", flag: "🇩🇪", name: "Alemão" },
  { mode: "translate-ar", short: "AR", flag: "🇸🇦", name: "Árabe" },
  { mode: "translate-bg", short: "BG", flag: "🇧🇬", name: "Búlgaro" },
  { mode: "translate-zh-cn", short: "ZH", flag: "🇨🇳", name: "Chinês (Simplificado)" },
  { mode: "translate-zh-tw", short: "ZH", flag: "🇹🇼", name: "Chinês (Tradicional)" },
  { mode: "translate-ko", short: "KO", flag: "🇰🇷", name: "Coreano" },
  { mode: "translate-hr", short: "HR", flag: "🇭🇷", name: "Croata" },
  { mode: "translate-da", short: "DA", flag: "🇩🇰", name: "Dinamarquês" },
  { mode: "translate-sk", short: "SK", flag: "🇸🇰", name: "Eslovaco" },
  { mode: "translate-sl", short: "SL", flag: "🇸🇮", name: "Esloveno" },
  { mode: "translate-es-us", short: "ES", flag: "🇲🇽", name: "Espanhol (América Latina)" },
  { mode: "translate-es-es", short: "ES", flag: "🇪🇸", name: "Espanhol (Espanha)" },
  { mode: "translate-et", short: "ET", flag: "🇪🇪", name: "Estoniano" },
  { mode: "translate-fi", short: "FI", flag: "🇫🇮", name: "Finlandês" },
  { mode: "translate-fr", short: "FR", flag: "🇫🇷", name: "Francês" },
  { mode: "translate-el", short: "EL", flag: "🇬🇷", name: "Grego" },
  { mode: "translate-hi", short: "HI", flag: "🇮🇳", name: "Hindi" },
  { mode: "translate-nl", short: "NL", flag: "🇳🇱", name: "Holandês" },
  { mode: "translate-hu", short: "HU", flag: "🇭🇺", name: "Húngaro" },
  { mode: "translate-id", short: "ID", flag: "🇮🇩", name: "Indonésio" },
  { mode: "translate-it", short: "IT", flag: "🇮🇹", name: "Italiano" },
  { mode: "translate-ja", short: "JA", flag: "🇯🇵", name: "Japonês" },
  { mode: "translate-lv", short: "LV", flag: "🇱🇻", name: "Letão" },
  { mode: "translate-lt", short: "LT", flag: "🇱🇹", name: "Lituano" },
  { mode: "translate-no", short: "NO", flag: "🇳🇴", name: "Norueguês" },
  { mode: "translate-pl", short: "PL", flag: "🇵🇱", name: "Polonês" },
  { mode: "translate-pt-pt", short: "PT", flag: "🇵🇹", name: "Português (Portugal)" },
  { mode: "translate-ro", short: "RO", flag: "🇷🇴", name: "Romeno" },
  { mode: "translate-ru", short: "RU", flag: "🇷🇺", name: "Russo" },
  { mode: "translate-sv", short: "SV", flag: "🇸🇪", name: "Sueco" },
  { mode: "translate-th", short: "TH", flag: "🇹🇭", name: "Tailandês" },
  { mode: "translate-cs", short: "CS", flag: "🇨🇿", name: "Tcheco" },
  { mode: "translate-tr", short: "TR", flag: "🇹🇷", name: "Turco" },
  { mode: "translate-uk", short: "UK", flag: "🇺🇦", name: "Ucraniano" },
  { mode: "translate-vi", short: "VI", flag: "🇻🇳", name: "Vietnamita" },
];
