import { t } from "./i18n";
import type { DictationEvent } from "./contracts";

/**
 * Indicador do ditado: mostra quando o microfone está aberto (regra de
 * produto: nunca gravar sem indicação), o texto parcial reconhecido e o tempo
 * decorrido no modo alternar. O sinal sonoro cobre jogos em tela cheia
 * exclusiva, onde a janela não aparece.
 */
export interface DictationFrame {
  hidden: boolean;
  phase: DictationEvent["phase"];
  compact: boolean;
  text: string;
  hint: string;
  elapsed: string;
}

export class DictationIndicator {
  private frame: DictationFrame = {
    hidden: true,
    phase: "idle",
    compact: false,
    text: "",
    hint: "",
    elapsed: "",
  };
  private timer: ReturnType<typeof setInterval> | undefined;
  private startedAt = 0;
  private lastPhase: DictationEvent["phase"] = "idle";
  private audio: AudioContext | undefined;

  constructor(
    private readonly onChange: (frame: DictationFrame) => void,
    private readonly showError: (message: string) => void,
  ) {}

  private publish(patch: Partial<DictationFrame>) {
    this.frame = { ...this.frame, ...patch };
    this.onChange(this.frame);
  }

  accept(event: DictationEvent) {
    if (event.phase === "recording" && this.lastPhase === "idle") {
      this.startedAt = Date.now();
      if (event.soundCue) this.beep(880);
      this.startTimer();
    }
    if (event.phase === "idle" && this.lastPhase !== "idle" && event.soundCue) {
      this.beep(440);
    }
    this.lastPhase = event.phase;
    if (event.phase === "idle") {
      this.stopTimer();
      this.publish({ hidden: true, phase: "idle" });
      if (event.error) this.showError(dictationErrorMessage(event.error));
      return;
    }
    const hint =
      event.phase === "finalizing"
        ? t("dictationFinalizing")
        : event.activation === "hold"
          ? t("dictationListening")
          : t("dictationListeningToggle");
    this.publish({
      hidden: false,
      phase: event.phase,
      compact: !event.showLiveText,
      text: event.showLiveText ? event.text || hint : "",
      hint,
    });
  }

  private startTimer() {
    this.stopTimer();
    this.renderElapsed();
    this.timer = setInterval(() => this.renderElapsed(), 1_000);
  }

  private stopTimer() {
    if (this.timer !== undefined) clearInterval(this.timer);
    this.timer = undefined;
    this.publish({ elapsed: "" });
  }

  private renderElapsed() {
    const total = Math.max(
      0,
      Math.round((Date.now() - this.startedAt) / 1_000),
    );
    const minutes = Math.floor(total / 60);
    const seconds = String(total % 60).padStart(2, "0");
    this.publish({ elapsed: `${minutes}:${seconds}` });
  }

  /** Bipe curto embutido (WebAudio); sem arquivos e sem rede. */
  private beep(frequency: number) {
    try {
      // jsdom e ambientes sem áudio não têm AudioContext.
      if (typeof AudioContext === "undefined") return;
      this.audio ??= new AudioContext();
      const oscillator = this.audio.createOscillator();
      const gain = this.audio.createGain();
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.08, this.audio.currentTime);
      gain.gain.exponentialRampToValueAtTime(
        0.0001,
        this.audio.currentTime + 0.12,
      );
      oscillator.connect(gain).connect(this.audio.destination);
      oscillator.start();
      oscillator.stop(this.audio.currentTime + 0.12);
    } catch {
      /* Sem áudio disponível; o indicador visual continua. */
    }
  }

  dispose() {
    this.stopTimer();
    void this.audio?.close().catch(() => undefined);
    this.audio = undefined;
  }
}

export function dictationErrorMessage(code: string): string {
  switch (code) {
    case "dictation-capture":
      return t("errDictationMic");
    case "dictation-model":
      return t("errDictationModel");
    case "dictation-paste":
      return t("errDictationPaste");
    case "dictation-translate":
      return t("errDictationTranslate");
    default:
      return t("errDictation");
  }
}
