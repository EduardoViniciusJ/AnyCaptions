import type { CSSProperties } from "react";
import type { Appearance } from "../contracts";
import {
  CAPTION_FONTS,
  DEFAULT_APPEARANCE,
  captionBackground,
  captionFontFamily,
  normalizeCaptionColor,
  panelBackground,
} from "../appearance";
import { t, type MessageKey } from "../i18n";
import { Field } from "./Field";
export function appearanceStyle(value: Appearance): CSSProperties {
  return {
    "--caption-base-size": `${value.fontSize}px`,
    "--caption-color": value.color,
    "--caption-partial-color": value.partialColor,
    "--caption-font": captionFontFamily(value.font),
    "--caption-background": captionBackground(value),
  } as CSSProperties;
}

type Change = (value: Appearance) => void;

/**
 * Puts one option back to its default without touching the others (the
 * footer's "Restore default" resets everything at once). Like the option
 * itself, it changes the draft; Save applies it.
 */
function ResetOption({
  field,
  id,
  label,
  value,
  onChange,
}: {
  field: keyof Appearance;
  id: string;
  label: string;
  value: Appearance;
  onChange: Change;
}) {
  const initial = DEFAULT_APPEARANCE[field];
  const current = value[field];
  const changed =
    typeof current === "string" && typeof initial === "string"
      ? current.trim().toUpperCase() !== initial.toUpperCase()
      : current !== initial;
  const name = `${t("restoreDefault")}: ${label}`;
  return (
    <button
      className="btn btn-ghost btn-xs setting-reset"
      id={`${id}-reset`}
      type="button"
      title={name}
      aria-label={name}
      disabled={!changed}
      onClick={() => onChange({ ...value, [field]: initial })}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M3 8a5 5 0 1 0 1.6-3.7M3 2.5v2.4h2.4" />
      </svg>
    </button>
  );
}

type ColorField = "color" | "partialColor" | "panelColor" | "backgroundColor";

/** A color: the native picker, the hexadecimal value and its reset. */
function ColorOption({
  field,
  id,
  label,
  hint,
  pickerLabel,
  hexLabel,
  value,
  onChange,
}: {
  field: ColorField;
  id: string;
  label: MessageKey;
  hint: MessageKey;
  pickerLabel: MessageKey;
  hexLabel: MessageKey;
  value: Appearance;
  onChange: Change;
}) {
  return (
    <Field id={id} label={t(label)} hint={t(hint)}>
      <div className="color-control setting-with-reset">
        <input
          id={id}
          type="color"
          value={normalizeCaptionColor(value[field]) ?? "#ffffff"}
          aria-label={t(pickerLabel)}
          onChange={(event) =>
            onChange({ ...value, [field]: event.target.value.toUpperCase() })
          }
        />
        <input
          className="input input-sm"
          id={`${id}-value`}
          value={value[field]}
          maxLength={7}
          spellCheck={false}
          aria-label={t(hexLabel)}
          aria-invalid={!normalizeCaptionColor(value[field])}
          onChange={(event) => onChange({ ...value, [field]: event.target.value })}
          onBlur={() => {
            const normalized = normalizeCaptionColor(value[field]);
            if (normalized) onChange({ ...value, [field]: normalized });
          }}
        />
        <ResetOption field={field} id={id} label={t(label)} value={value} onChange={onChange} />
      </div>
    </Field>
  );
}

/** A number picked on a slider, with its current value and its reset. */
function RangeOption({
  field,
  id,
  outputId,
  label,
  hint,
  unit,
  min,
  max,
  step,
  value,
  onChange,
}: {
  field: "fontSize" | "panelOpacity" | "backgroundOpacity";
  id: string;
  outputId: string;
  label: MessageKey;
  hint: MessageKey;
  unit: string;
  min: number;
  max: number;
  step: number;
  value: Appearance;
  onChange: Change;
}) {
  return (
    <Field id={id} label={t(label)} hint={t(hint)}>
      <div className="setting-with-reset">
        <div>
          <output id={outputId} htmlFor={id}>
            {value[field]}
            {unit}
          </output>
          <input
            className="range range-xs range-primary"
            id={id}
            type="range"
            min={min}
            max={max}
            step={step}
            value={value[field]}
            onChange={(event) =>
              onChange({ ...value, [field]: Number(event.target.value) })
            }
          />
        </div>
        <ResetOption field={field} id={id} label={t(label)} value={value} onChange={onChange} />
      </div>
    </Field>
  );
}

export function AppearanceSettings({
  value,
  onChange,
}: {
  value: Appearance;
  onChange: Change;
}) {
  return (
    <>
      <div
        className="appearance-preview"
        aria-label={t("previewAria")}
        style={{ "--panel-background": panelBackground(value) } as CSSProperties}
      >
        <span className="appearance-preview-label">{t("preview")}</span>
        <p
          id="appearance-preview"
          className="caption appearance-example"
          style={appearanceStyle(value)}
          data-background={value.background ? "" : undefined}
        >
          {t("previewFinal")}
          <span className="caption-partial">{t("previewPartial")}</span>
        </p>
      </div>
      <RangeOption
        field="fontSize"
        id="font-size"
        outputId="font-value"
        label="fontSize"
        hint="hintFontSize"
        unit=" px"
        min={16}
        max={80}
        step={1}
        value={value}
        onChange={onChange}
      />
      <ColorOption
        field="color"
        id="font-color"
        label="confirmedText"
        hint="hintConfirmedText"
        pickerLabel="pickConfirmedColor"
        hexLabel="confirmedColorHex"
        value={value}
        onChange={onChange}
      />
      <ColorOption
        field="partialColor"
        id="partial-color"
        label="recentWords"
        hint="hintRecentWords"
        pickerLabel="pickRecentColor"
        hexLabel="recentColorHex"
        value={value}
        onChange={onChange}
      />
      <Field id="caption-font" label={t("captionFont")} hint={t("hintCaptionFont")}>
        <div className="setting-with-reset">
          <select
            className="select select-sm"
            id="caption-font"
            value={value.font}
            style={{ fontFamily: captionFontFamily(value.font) }}
            onChange={(event) => onChange({ ...value, font: event.target.value })}
          >
            {CAPTION_FONTS.map((font) => (
              <option key={font.id} value={font.id} style={{ fontFamily: font.family }}>
                {font.name}
              </option>
            ))}
          </select>
          <ResetOption field="font" id="caption-font" label={t("captionFont")} value={value} onChange={onChange} />
        </div>
      </Field>
      <Field
        id="caption-background"
        label={t("captionBackground")}
        hint={t("hintCaptionBackground")}
      >
        <div className="setting-with-reset">
          <input
            className="toggle toggle-sm toggle-primary"
            id="caption-background"
            type="checkbox"
            checked={value.background}
            onChange={(event) =>
              onChange({ ...value, background: event.target.checked })
            }
          />
          <ResetOption field="background" id="caption-background" label={t("captionBackground")} value={value} onChange={onChange} />
        </div>
      </Field>
      {value.background && (
        // The band's own look only matters while the band is on.
        <>
          <ColorOption
            field="backgroundColor"
            id="background-color"
            label="backgroundColor"
            hint="hintBackgroundColor"
            pickerLabel="pickBackgroundColor"
            hexLabel="backgroundColorHex"
            value={value}
            onChange={onChange}
          />
          <RangeOption
            field="backgroundOpacity"
            id="background-opacity"
            outputId="background-opacity-value"
            label="backgroundOpacity"
            hint="hintBackgroundOpacity"
            unit="%"
            min={0}
            max={100}
            step={5}
            value={value}
            onChange={onChange}
          />
        </>
      )}
      <h3 className="settings-subtitle">{t("windowLook")}</h3>
      <ColorOption
        field="panelColor"
        id="panel-color"
        label="panelColor"
        hint="hintPanelColor"
        pickerLabel="pickPanelColor"
        hexLabel="panelColorHex"
        value={value}
        onChange={onChange}
      />
      <RangeOption
        field="panelOpacity"
        id="panel-opacity"
        outputId="panel-opacity-value"
        label="panelOpacity"
        hint="hintPanelOpacity"
        unit="%"
        min={0}
        max={100}
        step={5}
        value={value}
        onChange={onChange}
      />
    </>
  );
}
