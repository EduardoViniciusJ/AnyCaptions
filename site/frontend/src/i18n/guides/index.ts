import type { Lang } from '../index';
import type { GuideMessages } from './types';
import { ptBr } from './pt-br';
import { en } from './en';
import { es } from './es';
import { fr } from './fr';
import { ru } from './ru';
import { zhCn } from './zh-cn';

export { guideSlugs, isGuideSlug, type GuideSlug } from './types';

export const guideMessages: Record<Lang, GuideMessages> = {
  'pt-br': ptBr,
  en,
  es,
  fr,
  ru,
  'zh-cn': zhCn,
};
