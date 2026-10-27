import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { langNames, langs, type Lang } from '@/i18n';

const flags: Record<Lang, string> = {
  'pt-br': 'br',
  en: 'us',
  es: 'es',
  fr: 'fr',
  ru: 'ru',
  'zh-cn': 'cn',
};

// O cabeçalho calcula a URL de cada idioma no servidor; trocar é só navegar.
export function LanguagePicker({ lang, targets, label }: {
  lang: Lang;
  targets: Record<Lang, string>;
  label: string;
}) {
  return (
    <Select value={lang} onValueChange={value => { location.href = targets[value as Lang]; }}>
      <SelectTrigger className="lang-trigger nav-pill" aria-label={`${label}: ${langNames[lang]}`}>
        <SelectValue>
          <span className="flex items-center gap-2">
            <img className="lang-flag" src={`/flags/${flags[lang]}.svg`} alt="" width={20} height={14} />
            {langNames[lang]}
          </span>
        </SelectValue>
      </SelectTrigger>
      <SelectContent position="popper" align="end" className="z-50 min-w-52">
        {langs.map(item => (
          <SelectItem key={item} value={item} textValue={langNames[item]}>
            <span className="flex items-center gap-2">
              <img className="lang-flag" src={`/flags/${flags[item]}.svg`} alt="" width={20} height={14} />
              {langNames[item]}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
