import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { DictationEvent } from "../contracts";
import { setLocale } from "../i18n";
import { DictationIndicator } from "../components/DictationIndicator";

interface IndicatorSnapshot { revision: number; uiLanguage: string; event: DictationEvent }
/** Only the main overlay plays sounds and reports errors; this window is display-only. */
const source = {
  async subscribeDictation(listener: (event: DictationEvent) => void) {
    let revision = -1;
    const accept = (snapshot: IndicatorSnapshot | null) => {
      if (!snapshot || snapshot.revision <= revision) return;
      revision = snapshot.revision;
      setLocale(snapshot.uiLanguage);
      listener({ ...snapshot.event, soundCue: false, error: null });
    };
    const release = await getCurrentWindow().listen<IndicatorSnapshot>("dictation-indicator-changed", event => accept(event.payload));
    try { accept(await invoke<IndicatorSnapshot | null>("get_dictation_indicator")); }
    catch (error) { release(); throw error; }
    return release;
  },
};
const ignore = () => {};
export function DictationWindow() {
  return <main className="dictation-window" data-theme="anycaptions"><DictationIndicator backend={source} onError={ignore} /></main>;
}
