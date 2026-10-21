import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Backend, AppSnapshot, CaptionUpdate } from "./contracts";

export const backend: Backend = {
  getState: () => invoke("get_state"),
  setMode: (mode) => invoke("set_caption_mode", { request: { mode } }),
  start: () => invoke("start_capture"),
  stop: () => invoke("stop_capture"),
  setAppearance: (request) => invoke("set_appearance", { request }),
  setWindowMode: (mode) => invoke("set_window_mode", { request: { mode } }),
  setShortcut: (shortcut) => invoke("set_shortcut", { request: { shortcut } }),
  subscribe: (listener) => getCurrentWindow().listen<AppSnapshot>("app-state-changed", (event) => listener(event.payload)),
  subscribeCaptions: (listener) => getCurrentWindow().listen<CaptionUpdate>("caption-updated", (event) => listener(event.payload)),
  resize: () => getCurrentWindow().startResizeDragging("SouthEast"),
  close: () => getCurrentWindow().close(),
};
