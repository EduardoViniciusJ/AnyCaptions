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

/**
 * Idiomas do modelo ASR, espelhando `ASR_LANGUAGES` do backend: o dicionário
 * de prompts dentro do GGUF do Nemotron (chave `asr.rnnt.prompt_dictionary`,
 * um prompt por idioma; "auto" é o prompt de detecção). Usado pelo idioma do
 * ditado e pelo idioma de origem das legendas. O backend valida o valor;
 * esta tabela só apresenta. Ordem: PT-BR (padrão do público), EN, demais por nome.
 */
export interface AsrLanguage {
  tag: string;
  /** Código do país da bandeira (`assets/flags/<código>.svg`, flag-icons/MIT). */
  country: string;
  name: string;
}

export const ASR_LANGUAGES: AsrLanguage[] = [
  { tag: "pt-BR", country: "br", name: "Português (Brasil)" },
  { tag: "en-US", country: "us", name: "Inglês (EUA)" },
  { tag: "af-ZA", country: "za", name: "Africâner" },
  { tag: "ay-BO", country: "bo", name: "Aimará" },
  { tag: "de", country: "de", name: "Alemão" },
  { tag: "am-ET", country: "et", name: "Amárico" },
  { tag: "ar", country: "sa", name: "Árabe" },
  { tag: "hy-AM", country: "am", name: "Armênio" },
  { tag: "az-AZ", country: "az", name: "Azerbaijano" },
  { tag: "bn-IN", country: "in", name: "Bengali" },
  { tag: "bg", country: "bg", name: "Búlgaro" },
  { tag: "kn-IN", country: "in", name: "Canarês" },
  { tag: "ny-MW", country: "mw", name: "Chichewa" },
  { tag: "zh-CN", country: "cn", name: "Chinês (Simplificado)" },
  { tag: "zh-TW", country: "tw", name: "Chinês (Tradicional)" },
  { tag: "si-LK", country: "lk", name: "Cingalês" },
  { tag: "ko", country: "kr", name: "Coreano" },
  { tag: "hr", country: "hr", name: "Croata" },
  { tag: "ku-TR", country: "tr", name: "Curdo" },
  { tag: "da", country: "dk", name: "Dinamarquês" },
  { tag: "sk", country: "sk", name: "Eslovaco" },
  { tag: "sl", country: "si", name: "Esloveno" },
  { tag: "es-US", country: "mx", name: "Espanhol (América Latina)" },
  { tag: "es-ES", country: "es", name: "Espanhol (Espanha)" },
  { tag: "et", country: "ee", name: "Estoniano" },
  { tag: "fi", country: "fi", name: "Finlandês" },
  { tag: "fr", country: "fr", name: "Francês" },
  { tag: "fr-CA", country: "ca", name: "Francês (Canadá)" },
  { tag: "ka-GE", country: "ge", name: "Georgiano" },
  { tag: "el", country: "gr", name: "Grego" },
  { tag: "gn-PY", country: "py", name: "Guarani" },
  { tag: "gu-IN", country: "in", name: "Guzerate" },
  { tag: "ha-NG", country: "ng", name: "Hauçá" },
  { tag: "haw-US", country: "us", name: "Havaiano" },
  { tag: "he-IL", country: "il", name: "Hebraico" },
  { tag: "hi", country: "in", name: "Hindi" },
  { tag: "nl", country: "nl", name: "Holandês" },
  { tag: "hu", country: "hu", name: "Húngaro" },
  { tag: "ig-NG", country: "ng", name: "Igbo" },
  { tag: "id-ID", country: "id", name: "Indonésio" },
  { tag: "en-GB", country: "gb", name: "Inglês (Reino Unido)" },
  { tag: "yo-NG", country: "ng", name: "Iorubá" },
  { tag: "it", country: "it", name: "Italiano" },
  { tag: "ja-JP", country: "jp", name: "Japonês" },
  { tag: "km-KH", country: "kh", name: "Khmer" },
  { tag: "lv", country: "lv", name: "Letão" },
  { tag: "ln-CD", country: "cd", name: "Lingala" },
  { tag: "lt", country: "lt", name: "Lituano" },
  { tag: "ml-IN", country: "in", name: "Malaiala" },
  { tag: "ms-MY", country: "my", name: "Malaio" },
  { tag: "mt-MT", country: "mt", name: "Maltês" },
  { tag: "mi-NZ", country: "nz", name: "Maori" },
  { tag: "mr-IN", country: "in", name: "Marati" },
  { tag: "nah-MX", country: "mx", name: "Náuatle" },
  { tag: "ne-NP", country: "np", name: "Nepalês" },
  { tag: "no", country: "no", name: "Norueguês" },
  { tag: "nb", country: "no", name: "Norueguês (Bokmål)" },
  { tag: "nn", country: "no", name: "Norueguês (Nynorsk)" },
  { tag: "or-KE", country: "ke", name: "Oromo" },
  { tag: "fa-IR", country: "ir", name: "Persa" },
  { tag: "pl", country: "pl", name: "Polonês" },
  { tag: "pt-PT", country: "pt", name: "Português (Portugal)" },
  { tag: "qu-PE", country: "pe", name: "Quéchua" },
  { tag: "rw-RW", country: "rw", name: "Quiniaruanda" },
  { tag: "ky-KG", country: "kg", name: "Quirguiz" },
  { tag: "ro", country: "ro", name: "Romeno" },
  { tag: "ru", country: "ru", name: "Russo" },
  { tag: "sm-WS", country: "ws", name: "Samoano" },
  { tag: "so-SO", country: "so", name: "Somali" },
  { tag: "sw-KE", country: "ke", name: "Suaíli" },
  { tag: "sv", country: "se", name: "Sueco" },
  { tag: "tg-TJ", country: "tj", name: "Tadjique" },
  { tag: "th-TH", country: "th", name: "Tailandês" },
  { tag: "ta-IN", country: "in", name: "Tâmil" },
  { tag: "cs", country: "cz", name: "Tcheco" },
  { tag: "te-IN", country: "in", name: "Télugo" },
  { tag: "to-TO", country: "to", name: "Tonganês" },
  { tag: "tr", country: "tr", name: "Turco" },
  { tag: "uk", country: "ua", name: "Ucraniano" },
  { tag: "ur-PK", country: "pk", name: "Urdu" },
  { tag: "uz-UZ", country: "uz", name: "Uzbeque" },
  { tag: "vi-VN", country: "vn", name: "Vietnamita" },
  { tag: "zu-ZA", country: "za", name: "Zulu" },
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
