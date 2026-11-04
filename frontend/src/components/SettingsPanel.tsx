import { useEffect, useState } from "react";
import type {
  AppSnapshot,
  Backend,
  DictationSnapshot,
  OutputSelection,
} from "../contracts";
import {
  DEFAULT_APPEARANCE,
  normalizeCaptionColor,
  sameAppearance,
} from "../appearance";
import { ASR_LANGUAGES, TRANSLATION_ENGINES } from "../languages";
import { t, UI_LANGUAGES } from "../i18n";
import { AppearanceSettings } from "./AppearanceSettings";
import { DictationSettings } from "./DictationSettings";
import { LanguagePicker } from "./LanguagePicker";
import { ModelDownload, useModelDownloads } from "./ModelDownload";
import { UpdateSettings } from "./UpdateSettings";
import { Field } from "./Field";
export function SettingsPanel({
  backend,
  state,
  open,
  busy,
  perform,
  report,
  onClose,
}: {
  backend: Backend;
  state: AppSnapshot;
  open: boolean;
  busy: boolean;
  perform(action: () => Promise<AppSnapshot>): Promise<boolean>;
  report(error: unknown): void;
  onClose(): void;
}) {
  const [appearance, setAppearance] = useState(state.appearance);
  const [shortcut, setShortcut] = useState(state.shortcut);
  const [outputs, setOutputs] = useState<OutputSelection>();
  const [outputBusy, setOutputBusy] = useState(false);
  const [dictation, setDictation] = useState<DictationSnapshot>();
  const [draft, setDraft] = useState<DictationSnapshot["settings"]>();
  const downloads = useModelDownloads(backend);
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setAppearance(state.appearance);
    setShortcut(state.shortcut);
    setOutputBusy(true);
    void backend
      .getAudioOutputs()
      .then((value) => {
        if (!cancelled) setOutputs(value);
      })
      .catch((error) => {
        if (!cancelled) report(error);
      })
      .finally(() => {
        if (!cancelled) setOutputBusy(false);
      });
    void backend
      .getDictation()
      .then((value) => {
        if (!cancelled) {
          setDictation(value);
          setDraft(value.settings);
        }
      })
      .catch((error) => {
        if (!cancelled) report(error);
      });
    return () => {
      cancelled = true;
    };
    // Snapshot events must not overwrite unsaved edits while this panel is open.
  }, [backend, open]);
  const valid =
    !!normalizeCaptionColor(appearance.color) &&
    !!normalizeCaptionColor(appearance.partialColor);
  const save = async () => {
    if (!valid) return;
    const ok = await perform(async () => {
      let saved = state;
      const normalized = {
        ...appearance,
        color: normalizeCaptionColor(appearance.color)!,
        partialColor: normalizeCaptionColor(appearance.partialColor)!,
      };
      if (!sameAppearance(normalized, saved.appearance))
        saved = await backend.setAppearance(normalized);
      if (shortcut !== saved.shortcut)
        saved = await backend.setShortcut(shortcut);
      if (
        draft &&
        dictation &&
        JSON.stringify(draft) !== JSON.stringify(dictation.settings)
      ) {
        const result = await backend.setDictation({
          ...draft,
          shortcut: draft.shortcut.trim(),
        });
        setDictation(result);
        setDraft(result.settings);
      }
      return saved;
    });
    if (ok) onClose();
  };
  return (
    <section
      id="settings"
      className="settings"
      aria-label={t("settingsAria")}
      hidden={!open}
    >
      <h2>{t("settings")}</h2>
      <fieldset disabled={busy || outputBusy}>
        <AppearanceSettings value={appearance} onChange={setAppearance} />
        <Field id="ui-language" label={t("uiLanguage")}>
          <LanguagePicker
            id="ui-language"
            label={t("uiLanguage")}
            value={state.uiLanguage}
            options={UI_LANGUAGES.map((item) => ({
              value: item.tag,
              label: item.name,
              country: item.country,
            }))}
            onChange={(tag) => {
              if (tag !== state.uiLanguage)
                void perform(() => backend.setUiLanguage(tag));
            }}
          />
        </Field>
        <Field id="source-language" label={t("sourceLanguage")}>
          <LanguagePicker
            id="source-language"
            label={t("sourceLanguage")}
            value={state.sourceLanguage ?? ""}
            options={[
              { value: "", label: t("autoMode") },
              ...ASR_LANGUAGES.map((item) => ({
                value: item.tag,
                label: item.name,
                country: item.country,
              })),
            ]}
            onChange={(tag) => {
              if (tag !== (state.sourceLanguage ?? ""))
                void perform(() => backend.setSourceLanguage(tag || null));
            }}
          />
        </Field>
        <Field id="translation-engine" label={t("translationEngine")}>
          <select
            className="select select-sm"
            id="translation-engine"
            value={state.translationEngine}
            onChange={(event) => {
              const engine = event.target
                .value as AppSnapshot["translationEngine"];
              void perform(() => backend.setTranslationEngine(engine));
            }}
          >
            {TRANSLATION_ENGINES.map((engine) => (
              <option key={engine.id} value={engine.id}>
                {t(engine.labelKey)}
              </option>
            ))}
          </select>
        </Field>
      </fieldset>
      <ModelDownload
        backend={backend}
        downloads={downloads}
        engine={state.translationEngine}
        report={report}
      />
      <fieldset disabled={busy || outputBusy}>
        <h3 className="settings-subtitle">{t("speakerLabels")}</h3>
        <Field id="speaker-labels" label={t("speakerLabelsEnable")}>
          <input
            className="toggle toggle-sm toggle-primary"
            id="speaker-labels"
            type="checkbox"
            checked={state.speakerLabels}
            aria-describedby="speaker-labels-hint"
            onChange={(event) => {
              const enabled = event.target.checked;
              if (enabled !== state.speakerLabels)
                void perform(() => backend.setSpeakerLabels(enabled));
            }}
          />
        </Field>
        <p id="speaker-labels-hint" className="muted">
          {t("speakerLabelsHint")}
        </p>
      </fieldset>
      <ModelDownload
        backend={backend}
        downloads={downloads}
        kind="diarization"
        idPrefix="diarization-download"
        missingKey="speakerLabelsModelMissing"
        report={report}
      />
      <fieldset disabled={busy || outputBusy}>
        <Field id="audio-output" label={t("audioOutput")}>
          <select
            className="select select-sm"
            id="audio-output"
            disabled={!outputs?.devices.length}
            value={outputs?.selectedId ?? ""}
            onChange={(event) => {
              const id = event.target.value;
              if (!id || outputBusy) return;
              setOutputBusy(true);
              void backend
                .selectAudioOutput(id)
                .then(setOutputs)
                .catch(async (error) => {
                  report(error);
                  try {
                    setOutputs(await backend.getAudioOutputs());
                  } catch {
                    /* Keep previous selection. */
                  }
                })
                .finally(() => setOutputBusy(false));
            }}
          >
            {!outputs && <option value="">{t("checkingOutputs")}</option>}
            {outputs && !outputs.devices.length && (
              <option value="">{t("noOutputs")}</option>
            )}
            {!!outputs?.devices.length && !outputs.selectedId && (
              <option value="">{t("selectOutput")}</option>
            )}
            {outputs?.devices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.name}
              </option>
            ))}
            {outputs?.selectedId && !outputs.selectedAvailable && (
              <option value={outputs.selectedId}>
                {t("savedOutputUnavailable")}
              </option>
            )}
          </select>
        </Field>
        <p
          id="audio-output-status"
          className="muted"
          hidden={
            !outputs || (outputs.selectedAvailable && !!outputs.devices.length)
          }
        >
          {outputs?.selectedId && !outputs.selectedAvailable
            ? t("savedOutputDisconnected")
            : t("noActiveOutput")}
        </p>
        <Field id="shortcut" label={t("lockShortcut")}>
          <input
            className="input input-sm"
            id="shortcut"
            value={shortcut}
            onChange={(event) => setShortcut(event.target.value)}
            maxLength={16}
            spellCheck={false}
            aria-describedby="shortcut-status"
          />
        </Field>
        <p
          id="shortcut-status"
          className="muted"
          hidden={state.shortcutAvailable}
        >
          {t("shortcutUnavailableTray")}
        </p>
        {dictation && draft && (
          <DictationSettings
            value={draft}
            snapshot={dictation}
            engine={state.translationEngine}
            onChange={setDraft}
          />
        )}
      </fieldset>
      <UpdateSettings
        backend={backend}
        state={state}
        perform={perform}
        report={report}
      />
      <div className="settings-footer">
        <button
          className="btn btn-ghost btn-xs settings-credit"
          id="developer-credit"
          title="github.com/EduardoViniciusJ"
          onClick={() => void backend.openProjectPage().catch(report)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
          </svg>
          <span className="credit-prefix">{t("developedBy")}</span>
          {" "}
          <span className="credit-name">EduardoViniciusJ</span>
        </button>
        <button
          className="btn btn-sm"
          id="reset-appearance"
          disabled={busy || outputBusy}
          onClick={() => setAppearance({ ...DEFAULT_APPEARANCE })}
        >
          {t("restoreDefault")}
        </button>
        <button
          className="btn btn-sm btn-primary"
          id="save-settings"
          disabled={busy || outputBusy || !valid}
          onClick={() => void save()}
        >
          {t("save")}
        </button>
      </div>
    </section>
  );
}
