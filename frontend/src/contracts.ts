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
}

export interface AppSnapshot {
  revision: number;
  uiLanguage: string;
  sessionId: number;
  mode: CaptionMode;
  translationEngine: TranslationEngine;
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
  maxDurationSecs: number;
  inputId: string | null;
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
  maxDurationSecs: number;
}

export interface Backend {
  getState(): Promise<AppSnapshot>;
  setMode(mode: CaptionMode): Promise<AppSnapshot>;
  setTranslationEngine(engine: TranslationEngine): Promise<AppSnapshot>;
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
  subscribe(listener: (state: AppSnapshot) => void): Promise<() => void>;
  subscribeCaptions(listener: (update: CaptionUpdate) => void): Promise<() => void>;
  subscribeDictation(listener: (event: DictationEvent) => void): Promise<() => void>;
  resize(direction: ResizeDirection): Promise<void>;
  close(): Promise<void>;
}
