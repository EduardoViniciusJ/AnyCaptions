export type CaptionMode = "en-en" | "en-pt";
export type CaptureState = "stopped" | "preparing" | "listening" | "processing" | "error";
export type WindowMode = "edit" | "locked";
export type HorizontalResizeDirection = "East" | "West";
export type CaptionColor = `#${string}`;

export interface Appearance {
  fontSize: number;
  color: CaptionColor;
  partialColor: CaptionColor;
}

export interface AppSnapshot {
  revision: number;
  sessionId: number;
  mode: CaptionMode;
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
}

export interface OutputSelection {
  devices: { id: string; name: string }[];
  selectedId: string | null;
  selectedAvailable: boolean;
}

export interface Backend {
  getState(): Promise<AppSnapshot>;
  setMode(mode: CaptionMode): Promise<AppSnapshot>;
  start(): Promise<AppSnapshot>;
  stop(): Promise<AppSnapshot>;
  setAppearance(appearance: Appearance): Promise<AppSnapshot>;
  setWindowMode(mode: WindowMode): Promise<AppSnapshot>;
  setShortcut(shortcut: string): Promise<AppSnapshot>;
  getAudioOutputs(): Promise<OutputSelection>;
  selectAudioOutput(id: string): Promise<OutputSelection>;
  subscribe(listener: (state: AppSnapshot) => void): Promise<() => void>;
  subscribeCaptions(listener: (update: CaptionUpdate) => void): Promise<() => void>;
  resize(direction: HorizontalResizeDirection): Promise<void>;
  fitSettings(): Promise<void>;
  fitCaption(): Promise<void>;
  close(): Promise<void>;
}
