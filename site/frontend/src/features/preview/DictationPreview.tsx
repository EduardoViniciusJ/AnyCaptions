import { ArrowRight, Mic, Pause, Play } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { messages, type Lang } from '@/i18n';
import { usePreviewPlayback } from './usePreviewPlayback';

export function DictationPreview({ lang = 'pt-br' }: { lang?: Lang }) {
  const t = messages[lang].dictation;
  const { videoRef, playing, setPlaying, loading, togglePlayback } = usePreviewPlayback();
  const playbackLabel = playing ? t.pause : t.play;

  return (
    <section className="preview-section container" aria-labelledby="dictation-title">
      <div className="preview-heading">
        <p id="dictation-title">{t.heading}</p>
      </div>
      <Card className="caption-preview gap-0 overflow-hidden py-0">
        <div className="preview-topbar">
          <div className="preview-window">
            <span className="window-dots" aria-hidden="true"><i /><i /><i /></span>
            <span>{t.windowTitle}</span>
          </div>
          <div className="preview-controls dictation-source">
            <Mic size={14} aria-hidden="true" />
            <span>{t.mic}</span>
            <ArrowRight size={14} aria-hidden="true" />
            <span>{t.text}</span>
          </div>
        </div>
        <div className="preview-stage" aria-busy={loading}>
          <video
            ref={videoRef}
            className="preview-video"
            src="/media/codev2.mp4"
            poster="/media/codev2-poster.jpg"
            width={1920}
            height={1080}
            muted
            loop
            playsInline
            preload="metadata"
            aria-label={t.videoAria}
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
          />
          {loading && <Skeleton className="preview-skeleton" aria-hidden="true" />}
          <Button
            type="button"
            variant="secondary"
            size="icon"
            className="preview-video-control"
            onClick={togglePlayback}
            aria-label={playbackLabel}
            title={playbackLabel}
          >
            {playing ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
          </Button>
        </div>
      </Card>
    </section>
  );
}
