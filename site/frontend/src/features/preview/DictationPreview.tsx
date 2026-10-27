import { ArrowRight, Mic, Pause, Play } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { usePreviewPlayback } from './usePreviewPlayback';

export function DictationPreview() {
  const { videoRef, playing, setPlaying, togglePlayback } = usePreviewPlayback();
  const playbackLabel = playing ? 'Pausar demonstração de ditado' : 'Reproduzir demonstração de ditado';

  return (
    <section className="preview-section container" aria-labelledby="dictation-title">
      <div className="preview-heading">
        <p id="dictation-title">Transcreva o áudio do seu microfone em texto onde você escreve.</p>
      </div>
      <Card className="caption-preview gap-0 overflow-hidden py-0">
        <div className="preview-topbar">
          <div className="preview-window">
            <span className="window-dots" aria-hidden="true"><i /><i /><i /></span>
            <span>AnyCaptions / Ditado por voz</span>
          </div>
          <div className="preview-controls dictation-source">
            <Mic size={14} aria-hidden="true" />
            <span>Microfone</span>
            <ArrowRight size={14} aria-hidden="true" />
            <span>Texto</span>
          </div>
        </div>
        <div className="preview-stage">
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
            aria-label="Demonstração de ditado por voz em um campo de texto"
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
          />
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
