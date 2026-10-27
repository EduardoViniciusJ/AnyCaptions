import { useState } from 'react';
import { Pause, Play } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { messages, type Lang } from '@/i18n';
import { captions } from './captions';
import { usePreviewPlayback } from './usePreviewPlayback';

const languages = [
  { code: 'pt-BR', name: 'Português (Brasil)', flag: 'br' },
  { code: 'en', name: 'English', flag: 'us' },
  { code: 'es', name: 'Español', flag: 'es' },
  { code: 'zh', name: '中文（简体）', flag: 'cn' },
  { code: 'fr', name: 'Français', flag: 'fr' },
  { code: 'ru', name: 'Русский', flag: 'ru' },
] as const;

// A legenda da demonstração abre no idioma da página do site.
const initialCaption: Record<Lang, (typeof languages)[number]['code']> = {
  'pt-br': 'pt-BR',
  en: 'en',
  es: 'es',
  fr: 'fr',
  ru: 'ru',
  'zh-cn': 'zh',
};

export function CaptionPreview({ lang = 'pt-br' }: { lang?: Lang }) {
  const t = messages[lang].preview;
  const [language, setLanguage] = useState<string>(initialCaption[lang]);
  const [currentTime, setCurrentTime] = useState(0);
  const { videoRef, playing, setPlaying, reducedMotion, loading, togglePlayback } = usePreviewPlayback();
  const selected = languages.find(item => item.code === language)!;
  const words = language === 'zh' ? [...captions[language]] : captions[language].split(' ');
  const separator = language === 'zh' ? '' : ' ';
  const visibleCount = reducedMotion ? words.length : Math.min(words.length, Math.floor(currentTime / 0.23) + 1);
  const committedCount = visibleCount === words.length ? words.length : Math.max(0, visibleCount - 2);
  const committed = words.slice(0, committedCount).join(separator);
  const recent = words.slice(committedCount, visibleCount).join(separator);

  function changeLanguage(nextLanguage: string) {
    setLanguage(nextLanguage);
    setCurrentTime(0);
    if (videoRef.current) videoRef.current.currentTime = 0;
  }

  return (
    <section className="preview-section container" aria-label={t.sectionLabel}>
      <Card className="caption-preview gap-0 overflow-hidden py-0">
        <div className="preview-topbar">
          <div className="preview-window">
            <span className="window-dots" aria-hidden="true"><i /><i /><i /></span>
            <span>{t.windowTitle}</span>
          </div>
          <div className="preview-controls">
            <span className="source-language">{t.audio} <strong>EN</strong></span>
            <span className="language-arrow" aria-hidden="true">→</span>
            <Select value={language} onValueChange={changeLanguage}>
              <SelectTrigger className="preview-language-trigger" aria-label={`${t.captionLanguage} ${selected.name}`}>
                <SelectValue>
                  <span className="flex items-center gap-2">
                    <img className="preview-language-flag" src={`/flags/${selected.flag}.svg`} alt="" width={20} height={14} />
                    {selected.name}
                  </span>
                </SelectValue>
              </SelectTrigger>
              <SelectContent position="popper" align="end" className="z-50 min-w-60">
                {languages.map(item => (
                  <SelectItem key={item.code} value={item.code} textValue={item.name}>
                    <span className="flex items-center gap-2">
                      <img className="preview-language-flag" src={`/flags/${item.flag}.svg`} alt="" width={20} height={14} />
                      {item.name}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
        <div className="preview-stage" aria-busy={loading}>
          <video
            ref={videoRef}
            className="preview-video"
            src="/media/cs2v1.mp4"
            poster="/media/cs2v1-poster.jpg"
            width={1920}
            height={1080}
            muted
            loop
            playsInline
            preload="metadata"
            aria-label={t.videoAria}
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onTimeUpdate={event => setCurrentTime(event.currentTarget.currentTime)}
            onSeeked={event => setCurrentTime(event.currentTarget.currentTime)}
          />
          {loading && <Skeleton className="preview-skeleton" aria-hidden="true" />}
          <div className="preview-caption-area">
            <p className="preview-caption" lang={language}>
              <span>{committed}{committed && recent ? separator : ''}</span>
              <span className="caption-recent">{recent}</span>
            </p>
          </div>
          <Button
            type="button"
            variant="secondary"
            size="icon"
            className="preview-video-control"
            onClick={togglePlayback}
            aria-label={playing ? t.pause : t.play}
            title={playing ? t.pause : t.play}
          >
            {playing ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
          </Button>
        </div>
      </Card>
    </section>
  );
}
