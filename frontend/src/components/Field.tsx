import type { ReactNode } from "react";
import { t } from "../i18n";

/**
 * One settings row: the label, an optional "i" that explains the option, and
 * the control. The explanation opens on hover and on keyboard focus (a click
 * focuses it, which also covers touch) and Escape closes it; it is tied to
 * the button with `aria-describedby`, so screen readers read it too.
 */
export function Field({
  id,
  label,
  hint,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="setting-row">
      <span className="setting-label">
        <label htmlFor={id}>{label}</label>
        {hint && (
          <span className="setting-info">
            <button
              className="setting-info-button"
              id={`${id}-info`}
              type="button"
              aria-label={`${t("moreInfo")}: ${label}`}
              aria-describedby={`${id}-info-text`}
              onKeyDown={(event) => {
                if (event.key !== "Escape") return;
                // Only the explanation closes, not the whole settings panel.
                event.stopPropagation();
                event.currentTarget.blur();
              }}
            >
              i
            </button>
            <span className="setting-info-text" id={`${id}-info-text`} role="tooltip">
              {hint}
            </span>
          </span>
        )}
      </span>
      {children}
    </div>
  );
}
