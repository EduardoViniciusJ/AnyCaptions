export type CaptionMode = "auto" | `translate-${string}`;
export type TranslationEngine = "opus" | "riva";
export type CaptureState = "stopped" | "preparing" | "listening" | "processing" | "error";
export type WindowMode = "edit" | "locked";
export type ResizeDirection = "East" | "West" | "South";
export type CaptionColor = `#${string}`;

export interface Appearance {
  fontSize: number;
  color: CaptionColor;
  partialColor: CaptionColor;
  background: boolean;
}

export interface AppSnapshot {
  revision: number;
  uiLanguage: string;
  sessionId: number;
  mode: CaptionMode;
  translationEngine: TranslationEngine;
  /** Idioma falado fixado no reconhecedor (BCP-47); null = detecção automática. */
  sourceLanguage: string | null;
  capture: CaptureState;
  windowMode: WindowMode;
  appearance: Appearance;
  resources: { audioCapture: boolean; transcription: boolean; translation: boolean };
  canStart: boolean;
  canLock: boolean;
  shortcut: string;
  shortcutAvailable: boolean;
  error: string | null;
}

export interface CaptionUpdate {
  sessionId: number;
  segmentId: number;
  revision: number;
  mode: CaptionMode;
  text: string;
  isFinal: boolean;
  /** Idioma detectado pelo modelo (BCP-47); presente apenas em legendas finais. */
  language: string | null;
}

export interface OutputSelection {
  devices: { id: string; name: string }[];
  selectedId: string | null;
  selectedAvailable: boolean;
}

export type DictationActivation = "hold" | "toggle";
export type DictationPhase = "idle" | "recording" | "finalizing";

export interface DictationSettings {
  enabled: boolean;
  shortcut: string;
  activation: DictationActivation;
  soundCue: boolean;
  /** Mostrar o texto reconhecido no indicador; desligado, só ponto e tempo. */
  showLiveText: boolean;
  /** Aplicar o teto de duração; desligado, a gravação não é cortada. */
  limitDuration: boolean;
  maxDurationSecs: number;
  inputId: string | null;
  /** Idioma fixo do reconhecimento (BCP-47); null = detecção automática. */
  language: string | null;
  /** Alvo de tradução do ditado (tag do catálogo); null = colar como falado. */
  translateTo: string | null;
}

export interface DictationSnapshot {
  settings: DictationSettings;
  available: boolean;
  phase: DictationPhase;
  inputs: OutputSelection;
}

export interface DictationEvent {
  phase: DictationPhase;
  text: string;
  error: string | null;
  soundCue: boolean;
  activation: DictationActivation;
  showLiveText: boolean;
  maxDurationSecs: number;
}

/** Um modelo do catálogo que o app oferece para baixar nas configurações. */
export interface ModelDownloadState {
  model: string;
  engine: TranslationEngine;
  installed: boolean;
  downloadable: boolean;
  downloading: boolean;
  totalBytes: number;
}

export type ModelDownloadPhase = "downloading" | "ready" | "failed";
export type ModelDownloadFailure = "network" | "disk" | "corrupted" | "cancelled";

export interface ModelDownloadUpdate {
  model: string;
  phase: ModelDownloadPhase;
  receivedBytes: number;
  totalBytes: number | null;
  reason: ModelDownloadFailure | null;
}

export interface Backend {
  getState(): Promise<AppSnapshot>;
  setMode(mode: CaptionMode): Promise<AppSnapshot>;
  setTranslationEngine(engine: TranslationEngine): Promise<AppSnapshot>;
  setSourceLanguage(language: string | null): Promise<AppSnapshot>;
  setUiLanguage(language: string): Promise<AppSnapshot>;
  start(): Promise<AppSnapshot>;
  stop(): Promise<AppSnapshot>;
  setAppearance(appearance: Appearance): Promise<AppSnapshot>;
  setWindowMode(mode: WindowMode): Promise<AppSnapshot>;
  setShortcut(shortcut: string): Promise<AppSnapshot>;
  getAudioOutputs(): Promise<OutputSelection>;
  selectAudioOutput(id: string): Promise<OutputSelection>;
  getDictation(): Promise<DictationSnapshot>;
  setDictation(settings: DictationSettings): Promise<DictationSnapshot>;
  getModelDownloads(): Promise<ModelDownloadState[]>;
  downloadModel(model: string): Promise<void>;
  cancelModelDownload(): Promise<void>;
  subscribe(listener: (state: AppSnapshot) => void): Promise<() => void>;
  subscribeCaptions(listener: (update: CaptionUpdate) => void): Promise<() => void>;
  subscribeDictation(listener: (event: DictationEvent) => void): Promise<() => void>;
  subscribeModelDownloads(listener: (update: ModelDownloadUpdate) => void): Promise<() => void>;
  resize(direction: ResizeDirection): Promise<void>;
  close(): Promise<void>;
  openProjectPage(): Promise<void>;
}
