import type { SiteMessages } from './types';
import { ptBr } from './pt-br';
import { en } from './en';
import { es } from './es';
import { fr } from './fr';
import { ru } from './ru';
import { zhCn } from './zh-cn';

// Mesmos seis idiomas da interface do aplicativo. O português fica na raiz
// (sem prefixo); os demais vivem em /en, /es, /fr, /ru e /zh-cn.
export const langs = ['pt-br', 'en', 'es', 'fr', 'ru', 'zh-cn'] as const;
export type Lang = (typeof langs)[number];
export const defaultLang: Lang = 'pt-br';

export const messages: Record<Lang, SiteMessages> = {
  'pt-br': ptBr,
  en,
  es,
  fr,
  ru,
  'zh-cn': zhCn,
};

export const htmlLang: Record<Lang, string> = {
  'pt-br': 'pt-BR',
  en: 'en',
  es: 'es',
  fr: 'fr',
  ru: 'ru',
  'zh-cn': 'zh-CN',
};

export const ogLocale: Record<Lang, string> = {
  'pt-br': 'pt_BR',
  en: 'en_US',
  es: 'es_ES',
  fr: 'fr_FR',
  ru: 'ru_RU',
  'zh-cn': 'zh_CN',
};

// Nome de cada idioma escrito nele mesmo, para o seletor do cabeçalho.
export const langNames: Record<Lang, string> = {
  'pt-br': 'Português (Brasil)',
  en: 'English',
  es: 'Español',
  fr: 'Français',
  ru: 'Русский',
  'zh-cn': '中文（简体）',
};

export function isLang(value: string): value is Lang {
  return (langs as readonly string[]).includes(value);
}

/** Caminho local (começando com "/") traduzido para a URL do idioma. */
export function localePath(lang: Lang, path: string): string {
  if (lang === defaultLang) return path;
  return path === '/' ? `/${lang}/` : `/${lang}${path}`;
}

/** Remove o prefixo de idioma de um pathname, se houver. Barras repetidas
 * são normalizadas: um caminho que começasse com "//host" viraria uma URL
 * absoluta para outro site nos links de idioma (open redirect). */
export function stripLang(pathname: string): string {
  const normalized = pathname.replace(/\/{2,}/g, '/');
  const [, first, ...rest] = normalized.split('/');
  if (first && isLang(first) && first !== defaultLang) {
    const stripped = `/${rest.join('/')}`;
    return stripped === '' ? '/' : stripped;
  }
  return normalized === '' ? '/' : normalized;
}

/** Idioma de uma URL do site; caminhos sem prefixo são pt-BR. */
export function langFromUrl(url: URL): Lang {
  const [, first] = url.pathname.split('/');
  return first && isLang(first) ? first : defaultLang;
}
