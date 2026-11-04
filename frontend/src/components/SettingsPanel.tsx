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
import { ModelDownload } from "./ModelDownload";
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
        engine={state.translationEngine}
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
      <div className="settings-footer">
        <button
          className="btn btn-ghost btn-xs settings-credit"
          id="developer-credit"
          onClick={() => void backend.openProjectPage().catch(report)}
        >
          developed by EduardoViniciusJ
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
