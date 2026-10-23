import type { CaptionMode, TranslationEngine } from "./contracts";
import type { MessageKey } from "./i18n";

/**
 * Mecanismos de tradução oferecidos nas configurações, espelhando
 * `TranslationEngine` do backend (que valida a escolha e os alvos).
 */
export interface TranslationEngineOption {
  id: TranslationEngine;
  /** Rótulo traduzível com nome e características do mecanismo. */
  labelKey: MessageKey;
}

export const TRANSLATION_ENGINES: TranslationEngineOption[] = [
  { id: "opus", labelKey: "engineOpus" },
  { id: "riva", labelKey: "engineRiva" },
];

/** Modos servidos por cada mecanismo; o backend valida, esta tabela só filtra o menu. */
const ENGINE_MODES: Record<TranslationEngine, "all" | ReadonlySet<CaptionMode>> = {
  opus: new Set<CaptionMode>(["translate-en", "translate-pt-br", "translate-pt-pt"]),
  riva: "all",
};

export function engineServes(engine: TranslationEngine, mode: CaptionMode): boolean {
  const modes = ENGINE_MODES[engine] ?? "all";
  return modes === "all" || modes.has(mode);
}

/**
 * Idiomas-alvo de tradução, espelhando `TRANSLATION_TARGETS` do backend
 * (a lista oficial do modelo). O backend valida o valor; esta tabela só
 * apresenta. Ordem: PT-BR (padrão), EN, demais por nome.
 */
export interface TranslationTarget {
  mode: CaptionMode;
  /** Sigla curta mostrada no rótulo (variantes se distinguem pela bandeira). */
  short: string;
  /** Código do país da bandeira (`assets/flags/<código>.svg`, flag-icons/MIT). */
  country: string;
  name: string;
}

export const TRANSLATION_TARGETS: TranslationTarget[] = [
  { mode: "translate-pt-br", short: "PT", country: "br", name: "Português (Brasil)" },
  { mode: "translate-en", short: "EN", country: "us", name: "Inglês" },
  { mode: "translate-de", short: "DE", country: "de", name: "Alemão" },
  { mode: "translate-ar", short: "AR", country: "sa", name: "Árabe" },
  { mode: "translate-bg", short: "BG", country: "bg", name: "Búlgaro" },
  { mode: "translate-zh-cn", short: "ZH", country: "cn", name: "Chinês (Simplificado)" },
  { mode: "translate-zh-tw", short: "ZH", country: "tw", name: "Chinês (Tradicional)" },
  { mode: "translate-ko", short: "KO", country: "kr", name: "Coreano" },
  { mode: "translate-hr", short: "HR", country: "hr", name: "Croata" },
  { mode: "translate-da", short: "DA", country: "dk", name: "Dinamarquês" },
  { mode: "translate-sk", short: "SK", country: "sk", name: "Eslovaco" },
  { mode: "translate-sl", short: "SL", country: "si", name: "Esloveno" },
  { mode: "translate-es-us", short: "ES", country: "mx", name: "Espanhol (América Latina)" },
  { mode: "translate-es-es", short: "ES", country: "es", name: "Espanhol (Espanha)" },
  { mode: "translate-et", short: "ET", country: "ee", name: "Estoniano" },
  { mode: "translate-fi", short: "FI", country: "fi", name: "Finlandês" },
  { mode: "translate-fr", short: "FR", country: "fr", name: "Francês" },
  { mode: "translate-el", short: "EL", country: "gr", name: "Grego" },
  { mode: "translate-hi", short: "HI", country: "in", name: "Hindi" },
  { mode: "translate-nl", short: "NL", country: "nl", name: "Holandês" },
  { mode: "translate-hu", short: "HU", country: "hu", name: "Húngaro" },
  { mode: "translate-id", short: "ID", country: "id", name: "Indonésio" },
  { mode: "translate-it", short: "IT", country: "it", name: "Italiano" },
  { mode: "translate-ja", short: "JA", country: "jp", name: "Japonês" },
  { mode: "translate-lv", short: "LV", country: "lv", name: "Letão" },
  { mode: "translate-lt", short: "LT", country: "lt", name: "Lituano" },
  { mode: "translate-no", short: "NO", country: "no", name: "Norueguês" },
  { mode: "translate-pl", short: "PL", country: "pl", name: "Polonês" },
  { mode: "translate-pt-pt", short: "PT", country: "pt", name: "Português (Portugal)" },
  { mode: "translate-ro", short: "RO", country: "ro", name: "Romeno" },
  { mode: "translate-ru", short: "RU", country: "ru", name: "Russo" },
  { mode: "translate-sv", short: "SV", country: "se", name: "Sueco" },
  { mode: "translate-th", short: "TH", country: "th", name: "Tailandês" },
  { mode: "translate-cs", short: "CS", country: "cz", name: "Tcheco" },
  { mode: "translate-tr", short: "TR", country: "tr", name: "Turco" },
  { mode: "translate-uk", short: "UK", country: "ua", name: "Ucraniano" },
  { mode: "translate-vi", short: "VI", country: "vn", name: "Vietnamita" },
];

const flagUrls = import.meta.glob("./assets/flags/*.svg", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

/** URL empacotada da bandeira de um país, ou `undefined` se não existir. */
export function flagUrl(country: string): string | undefined {
  return flagUrls[`./assets/flags/${country}.svg`];
}
