import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Backend, AppSnapshot, CaptionUpdate, DictationEvent } from "./contracts";

export const backend: Backend = {
  getState: () => invoke("get_state"),
  setMode: (mode) => invoke("set_caption_mode", { request: { mode } }),
  setTranslationEngine: (engine) => invoke("set_translation_engine", { request: { engine } }),
  setUiLanguage: (language) => invoke("set_ui_language", { request: { language } }),
  start: () => invoke("start_capture"),
  stop: () => invoke("stop_capture"),
  setAppearance: (request) => invoke("set_appearance", { request }),
  setWindowMode: (mode) => invoke("set_window_mode", { request: { mode } }),
  setShortcut: (shortcut) => invoke("set_shortcut", { request: { shortcut } }),
  getAudioOutputs: () => invoke("get_audio_outputs"),
  selectAudioOutput: (id) => invoke("select_audio_output", { request: { id } }),
  getDictation: () => invoke("get_dictation"),
  setDictation: (request) => invoke("set_dictation", { request }),
  subscribe: (listener) => getCurrentWindow().listen<AppSnapshot>("app-state-changed", (event) => listener(event.payload)),
  subscribeCaptions: (listener) => getCurrentWindow().listen<CaptionUpdate>("caption-updated", (event) => listener(event.payload)),
  subscribeDictation: (listener) => getCurrentWindow().listen<DictationEvent>("dictation-changed", (event) => listener(event.payload)),
  resize: (direction) => getCurrentWindow().startResizeDragging(direction),
  close: () => getCurrentWindow().close(),
};
