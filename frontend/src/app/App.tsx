import { useEffect, useRef, useState } from "react";
import type { Backend, CaptionMode } from "../contracts";
import { TRANSLATION_TARGETS, engineServes } from "../languages";
import { t } from "../i18n";
import { errorMessage } from "./errors";
import { useApplication } from "./useApplication";
import { CaptionView } from "../components/CaptionView";
import { DictationIndicator } from "../components/DictationIndicator";
import { LanguagePicker } from "../components/LanguagePicker";
import { SettingsPanel } from "../components/SettingsPanel";
export function App({ backend }: { backend: Backend }) {
  const { state, busy, error, perform, report, setError } =
    useApplication(backend);
  const [settings, setSettings] = useState(false);
  const toggle = useRef<HTMLButtonElement>(null);
  const closeSettings = () => {
    setSettings(false);
    toggle.current?.focus();
  };
  useEffect(() => {
    if (state?.windowMode === "locked") setSettings(false);
  }, [state?.windowMode]);
  const stopped = !state || state.capture === "stopped";
  return (
    <section
      className="overlay"
      data-theme="anycaptions"
      data-mode={state?.windowMode ?? "edit"}
      data-settings={settings ? "open" : "closed"}
      aria-label={t("captionWindow")}
      onKeyDown={(event) => {
        if (event.key === "Escape" && settings) closeSettings();
      }}
    >
      <header className="toolbar">
        <div className="brand" data-tauri-drag-region title={t("dragMove")}>
          <span
            className="brand-mark"
            data-tauri-drag-region
            aria-hidden="true"
          >
            cc
          </span>
          <span className="brand-name" data-tauri-drag-region>
            AnyCaptions
          </span>
        </div>
        <div
          className="drag-region"
          data-tauri-drag-region
          title={t("dragMove")}
        />
        <div className="controls" aria-label={t("captionControls")}>
          <button
            className="btn btn-sm capture-button"
            id="capture"
            disabled={busy || !state || (stopped && !state.canStart)}
            title={stopped && !state?.canStart ? t("captureHint") : undefined}
            onClick={() =>
              void perform(() => (stopped ? backend.start() : backend.stop()))
            }
          >
            <span aria-hidden="true">{stopped ? "▶" : "■"}</span>
            <span id="capture-label">{t(stopped ? "start" : "stop")}</span>
          </button>
          <LanguagePicker
            id="language"
            label={t("captionLanguage")}
            value={state?.mode ?? "auto"}
            disabled={busy || !state || state.windowMode === "locked"}
            options={[
              { value: "auto", label: t("autoMode"), name: t("autoModeHint") },
              ...TRANSLATION_TARGETS.map((target) => ({
                value: target.mode,
                label: `${t("translateTo")} ${target.short}`,
                name: target.name,
                country: target.country,
                disabled: !state?.resources.translation,
                hidden: !engineServes(
                  state?.translationEngine ?? "opus",
                  target.mode,
                ),
              })),
            ]}
            onChange={(mode) => {
              if (mode !== state?.mode)
                void perform(() => backend.setMode(mode as CaptionMode));
            }}
          />
          <button
            className="btn btn-sm btn-square icon-button"
            id="settings-toggle"
            ref={toggle}
            disabled={!state}
            aria-label={t("settings")}
            title={t("settings")}
            aria-expanded={settings}
            aria-controls="settings"
            onClick={() => setSettings(!settings)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 7h16M4 17h16" />
              <circle cx="9" cy="7" r="3" />
              <circle cx="15" cy="17" r="3" />
            </svg>
          </button>
          <button
            className="btn btn-sm btn-square icon-button"
            id="lock"
            disabled={busy || !state?.canLock}
            aria-label={t("lockWindow")}
            title={t(state?.canLock ? "lockWindow" : "lockHint")}
            onClick={() => void perform(() => backend.setWindowMode("locked"))}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <rect x="5" y="10" width="14" height="11" rx="2" />
              <path d="M8 10V7a4 4 0 0 1 8 0" />
            </svg>
          </button>
        </div>
        <button
          className="btn btn-sm btn-ghost icon-button close-button"
          id="close"
          aria-label={t("closeToTray")}
          title={t("closeToTray")}
          onClick={() => void backend.close().catch(report)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m6 6 12 12M18 6 6 18" />
          </svg>
        </button>
      </header>
      <div className="content">
        <CaptionView
          backend={backend}
          state={state}
          hidden={settings}
          onError={setError}
        />
        {state && (
          <SettingsPanel
            backend={backend}
            state={state}
            open={settings}
            busy={busy}
            perform={perform}
            report={report}
            onClose={closeSettings}
          />
        )}
      </div>
      <DictationIndicator backend={backend} onError={setError} feedbackOnly />
      <p
        id="error"
        className="error"
        role="alert"
        hidden={!error && !state?.error}
      >
        {error || (state?.error ? errorMessage({ code: state.error }) : "")}
      </p>
      {(
        [
          ["left", "West"],
          ["right", "East"],
          ["bottom", "South"],
        ] as const
      ).map(([side, direction]) => (
        <div
          key={side}
          id={`resize-${side}`}
          className={`resize-grip resize-grip-${side}`}
          title={t(side === "bottom" ? "dragHeight" : "dragWidth")}
          aria-hidden="true"
          onMouseDown={(event) => {
            if (event.button !== 0) return;
            event.preventDefault();
            void backend.resize(direction).catch(report);
          }}
        />
      ))}
    </section>
  );
}
