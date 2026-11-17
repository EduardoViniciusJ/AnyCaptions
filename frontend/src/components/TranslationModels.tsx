import type { Backend, TranslationEngine } from "../contracts";
import { TRANSLATION_ENGINES } from "../languages";
import { t } from "../i18n";
import { ModelDownload, type ModelDownloads } from "./ModelDownload";

/**
 * The translation models as a list of choices instead of a dropdown, so the
 * differences are visible before picking: what each one translates, whether
 * it runs on the processor (CPU) or on the graphics card (GPU), and whether
 * it is already on disk. Each downloadable model is downloaded and removed
 * right on its own card.
 */
export function TranslationModels({
  backend,
  downloads,
  engine,
  disabled,
  onSelect,
  report,
}: {
  backend: Backend;
  downloads: ModelDownloads;
  engine: TranslationEngine;
  disabled: boolean;
  onSelect(engine: TranslationEngine): void;
  report(error: unknown): void;
}) {
  return (
    <>
      <p id="translation-engine-hint" className="muted settings-hint">
        {t("engineHint")}
      </p>
      <div
        id="translation-engine"
        className="engine-list"
        role="radiogroup"
        aria-label={t("translationEngine")}
        aria-describedby="translation-engine-hint"
      >
        {TRANSLATION_ENGINES.map((option) => {
          const selected = option.id === engine;
          return (
            <div
              key={option.id}
              className="engine-card"
              data-engine={option.id}
              data-selected={selected}
            >
              <label className="engine-choice">
                <input
                  className="radio radio-sm radio-primary"
                  type="radio"
                  name="translation-engine"
                  value={option.id}
                  checked={selected}
                  disabled={disabled}
                  aria-describedby={`engine-${option.id}-description`}
                  onChange={() => {
                    if (!selected) onSelect(option.id);
                  }}
                />
                <span className="engine-name">{option.name}</span>
                <span className="engine-device" data-device={option.device}>
                  {option.device.toUpperCase()}
                </span>
              </label>
              <p id={`engine-${option.id}-description`} className="muted">
                {t(option.descriptionKey)}
              </p>
              {option.bundled ? (
                <p className="muted engine-bundled">{t("modelIncluded")}</p>
              ) : (
                <ModelDownload
                  backend={backend}
                  downloads={downloads}
                  engine={option.id}
                  idPrefix={`model-${option.id}`}
                  report={report}
                />
              )}
            </div>
          );
        })}
      </div>
    </>
  );
}
