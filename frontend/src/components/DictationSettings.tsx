import type {
  DictationSettings as Settings,
  DictationSnapshot,
  TranslationEngine,
} from "../contracts";
import { ASR_LANGUAGES, TRANSLATION_TARGETS, engineServes } from "../languages";
import { t } from "../i18n";
import { Field } from "./Field";
import { LanguagePicker } from "./LanguagePicker";
export function DictationSettings({
  value,
  snapshot,
  engine,
  onChange,
}: {
  value: Settings;
  snapshot: DictationSnapshot;
  engine: TranslationEngine;
  onChange(value: Settings): void;
}) {
  return (
    <>
      <h3 className="settings-subtitle">{t("dictation")}</h3>
      {(
        [
          ["enabled", "dictation-enabled", "dictationEnable"],
          ["soundCue", "dictation-sound", "dictationSound"],
          ["showLiveText", "dictation-show-text", "dictationShowText"],
          ["limitDuration", "dictation-limit", "dictationLimitDuration"],
        ] as const
      ).map(([key, id, label]) => (
        <Field key={key} id={id} label={t(label)}>
          <input
            className="toggle toggle-sm toggle-primary"
            id={id}
            type="checkbox"
            checked={value[key]}
            onChange={(event) =>
              onChange({ ...value, [key]: event.target.checked })
            }
          />
        </Field>
      ))}
      <Field id="dictation-mic" label={t("dictationMic")}>
        <select
          className="select select-sm"
          id="dictation-mic"
          value={value.inputId ?? ""}
          onChange={(event) =>
            onChange({ ...value, inputId: event.target.value || null })
          }
        >
          <option value="">{t("dictationDefaultMic")}</option>
          {snapshot.inputs.devices.map((device) => (
            <option key={device.id} value={device.id}>
              {device.name}
            </option>
          ))}
          {value.inputId &&
            !snapshot.inputs.devices.some(
              (device) => device.id === value.inputId,
            ) && (
              <option value={value.inputId}>
                {t("savedOutputUnavailable")}
              </option>
            )}
        </select>
      </Field>
      <Field id="dictation-shortcut" label={t("dictationShortcut")}>
        <input
          className="input input-sm"
          id="dictation-shortcut"
          value={value.shortcut}
          maxLength={24}
          spellCheck={false}
          onChange={(event) =>
            onChange({ ...value, shortcut: event.target.value })
          }
        />
      </Field>
      <Field id="dictation-language" label={t("dictationLanguage")}>
        <LanguagePicker
          id="dictation-language"
          label={t("dictationLanguage")}
          value={value.language ?? ""}
          onChange={(language) =>
            onChange({ ...value, language: language || null })
          }
          options={[
            { value: "", label: t("autoMode") },
            ...ASR_LANGUAGES.map((item) => ({
              value: item.tag,
              label: item.name,
              country: item.country,
            })),
          ]}
        />
      </Field>
      <Field id="dictation-translate" label={t("dictationTranslate")}>
        <LanguagePicker
          id="dictation-translate"
          label={t("dictationTranslate")}
          value={value.translateTo ?? ""}
          onChange={(translateTo) =>
            onChange({ ...value, translateTo: translateTo || null })
          }
          options={[
            { value: "", label: t("dictationNoTranslate") },
            ...TRANSLATION_TARGETS.map((item) => ({
              value: item.mode.slice(10),
              label: item.name,
              country: item.country,
              hidden: !engineServes(engine, item.mode),
            })),
          ]}
        />
      </Field>
      <Field id="dictation-activation" label={t("dictationActivation")}>
        <select
          className="select select-sm"
          id="dictation-activation"
          value={value.activation}
          onChange={(event) =>
            onChange({
              ...value,
              activation: event.target.value as Settings["activation"],
            })
          }
        >
          <option value="hold">{t("dictationHold")}</option>
          <option value="toggle">{t("dictationToggle")}</option>
        </select>
      </Field>
      <Field id="dictation-duration" label={t("dictationMaxDuration")}>
        <input
          className="input input-sm"
          id="dictation-duration"
          type="number"
          min={5}
          max={300}
          step={5}
          disabled={!value.limitDuration}
          value={value.maxDurationSecs}
          onChange={(event) =>
            onChange({ ...value, maxDurationSecs: Number(event.target.value) })
          }
        />
      </Field>
      <p id="dictation-status" className="muted" hidden={snapshot.available}>
        {t("dictationModelMissing")}
      </p>
    </>
  );
}
