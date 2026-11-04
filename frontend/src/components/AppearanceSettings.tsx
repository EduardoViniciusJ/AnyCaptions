import type { CSSProperties } from "react";
import type { Appearance } from "../contracts";
import { CAPTION_FONTS, captionFontFamily, normalizeCaptionColor } from "../appearance";
import { t } from "../i18n";
import { Field } from "./Field";
export function appearanceStyle(value: Appearance): CSSProperties {
  return {
    "--caption-base-size": `${value.fontSize}px`,
    "--caption-color": value.color,
    "--caption-partial-color": value.partialColor,
    "--caption-font": captionFontFamily(value.font),
  } as CSSProperties;
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
      <div className="appearance-preview" aria-label={t("previewAria")}>
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
        ] as const
      ).map(([key, id, label, pickerLabel, hexLabel]) => (
        <Field key={key} id={id} label={t(label)}>
          <div className="color-control">
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
          </div>
        </Field>
      ))}
      <Field id="caption-font" label={t("captionFont")}>
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
      </Field>
      <Field id="caption-background" label={t("captionBackground")}>
        <input
          className="toggle toggle-sm toggle-primary"
          id="caption-background"
          type="checkbox"
          checked={value.background}
          onChange={(event) =>
            onChange({ ...value, background: event.target.checked })
          }
        />
      </Field>
    </>
  );
}
