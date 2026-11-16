import type { CSSProperties } from "react";
import type { Appearance } from "../contracts";
import {
  CAPTION_FONTS,
  DEFAULT_APPEARANCE,
  captionFontFamily,
  normalizeCaptionColor,
  panelBackground,
} from "../appearance";
import { t } from "../i18n";
import { Field } from "./Field";
export function appearanceStyle(value: Appearance): CSSProperties {
  return {
    "--caption-base-size": `${value.fontSize}px`,
    "--caption-color": value.color,
    "--caption-partial-color": value.partialColor,
    "--caption-font": captionFontFamily(value.font),
    "--panel-background": panelBackground(value),
  } as CSSProperties;
}
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
  onChange(value: Appearance): void;
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

export function AppearanceSettings({
  value,
  onChange,
}: {
  value: Appearance;
  onChange(value: Appearance): void;
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
      <Field id="font-size" label={t("fontSize")}>
        <div className="setting-with-reset">
        <div>
          <output id="font-value" htmlFor="font-size">
            {value.fontSize} px
          </output>
          <input
            className="range range-xs range-primary"
            id="font-size"
            type="range"
            min={16}
            max={80}
            step={1}
            value={value.fontSize}
            onChange={(event) =>
              onChange({ ...value, fontSize: Number(event.target.value) })
            }
          />
        </div>
          <ResetOption field="fontSize" id="font-size" label={t("fontSize")} value={value} onChange={onChange} />
        </div>
      </Field>
      {(
        [
          [
            "color",
            "font-color",
            "confirmedText",
            "pickConfirmedColor",
            "confirmedColorHex",
          ],
          [
            "partialColor",
            "partial-color",
            "recentWords",
            "pickRecentColor",
            "recentColorHex",
          ],
          [
            "panelColor",
            "panel-color",
            "panelColor",
            "pickPanelColor",
            "panelColorHex",
          ],
        ] as const
      ).map(([key, id, label, pickerLabel, hexLabel]) => (
        <Field key={key} id={id} label={t(label)}>
          <div className="color-control setting-with-reset">
            <input
              id={id}
              type="color"
              value={normalizeCaptionColor(value[key]) ?? "#ffffff"}
              aria-label={t(pickerLabel)}
              onChange={(event) =>
                onChange({ ...value, [key]: event.target.value.toUpperCase() })
              }
            />
            <input
              className="input input-sm"
              id={`${id}-value`}
              value={value[key]}
              maxLength={7}
              spellCheck={false}
              aria-label={t(hexLabel)}
              aria-invalid={!normalizeCaptionColor(value[key])}
              onChange={(event) =>
                onChange({ ...value, [key]: event.target.value })
              }
              onBlur={() => {
                const normalized = normalizeCaptionColor(value[key]);
                if (normalized) onChange({ ...value, [key]: normalized });
              }}
            />
            <ResetOption field={key} id={id} label={t(label)} value={value} onChange={onChange} />
          </div>
        </Field>
      ))}
      <Field id="panel-opacity" label={t("panelOpacity")}>
        <div className="setting-with-reset">
        <div>
          <output id="panel-opacity-value" htmlFor="panel-opacity">
            {value.panelOpacity}%
          </output>
          <input
            className="range range-xs range-primary"
            id="panel-opacity"
            type="range"
            min={0}
            max={100}
            step={5}
            value={value.panelOpacity}
            onChange={(event) =>
              onChange({ ...value, panelOpacity: Number(event.target.value) })
            }
          />
        </div>
          <ResetOption field="panelOpacity" id="panel-opacity" label={t("panelOpacity")} value={value} onChange={onChange} />
        </div>
      </Field>
      <Field id="caption-font" label={t("captionFont")}>
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
      <Field id="caption-background" label={t("captionBackground")}>
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
    </>
  );
}
