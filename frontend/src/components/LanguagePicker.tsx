import { useEffect, useRef, useState } from "react";
import { flagUrl } from "../languages";

export interface LanguageOption {
  value: string;
  label: string;
  name?: string;
  country?: string;
  disabled?: boolean;
  hidden?: boolean;
}
/** Linha final do menu que não é uma opção: uma dica com ação (ex.: baixar outro modelo). */
export interface PickerFooter {
  label: string;
  onSelect(): void;
}
export function LanguagePicker({
  id,
  label,
  value,
  options,
  disabled,
  footer,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  options: LanguageOption[];
  disabled?: boolean;
  footer?: PickerFooter;
  onChange(value: string): void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const selected =
    options.find((option) => option.value === value) ?? options[0];
  const close = () => {
    setOpen(false);
    trigger.current?.focus();
  };
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  const flag = (option: LanguageOption | undefined) => {
    const url = option?.country && flagUrl(option.country);
    return url ? (
      <img className="flag" src={url} alt="" />
    ) : (
      <span className="flag flag-globe" aria-hidden="true">
        ◎
      </span>
    );
  };
  return (
    <div
      className="language-picker"
      ref={root}
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.stopPropagation();
          close();
        }
        if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
          event.preventDefault();
          setOpen(true);
          requestAnimationFrame(() => {
            const items = [
              ...(root.current?.querySelectorAll<HTMLElement>(
                '[role="option"]:not([hidden]):not([aria-disabled="true"])',
              ) ?? []),
            ];
            const index = items.indexOf(document.activeElement as HTMLElement);
            const next =
              event.key === "Home"
                ? 0
                : event.key === "End"
                  ? items.length - 1
                  : (index +
                      (event.key === "ArrowUp" ? -1 : 1) +
                      items.length) %
                    items.length;
            items[next]?.focus();
          });
        }
      }}
    >
      <button
        className="btn btn-sm"
        id={id}
        ref={trigger}
        type="button"
        disabled={disabled}
        aria-label={label}
        aria-haspopup="listbox"
        aria-controls={`${id}-menu`}
        aria-expanded={open && !disabled}
        title={selected?.name}
        onClick={() => setOpen(!open)}
      >
        {flag(selected)}
        <span id={`${id}-label`}>{selected?.label}</span>
      </button>
      <ul
        id={`${id}-menu`}
        role="listbox"
        aria-label={label}
        hidden={!open || disabled}
      >
        {options.map((option) => (
          <li
            key={option.value}
            role="option"
            tabIndex={-1}
            data-value={option.value}
            hidden={option.hidden}
            aria-selected={value === option.value}
            aria-disabled={!!option.disabled}
            title={option.name}
            onClick={() => {
              if (!disabled && !option.disabled && !option.hidden) {
                onChange(option.value);
                close();
              }
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                event.currentTarget.click();
              }
            }}
          >
            {flag(option)}
            <span>{option.label}</span>
          </li>
        ))}
        {footer && (
          <li className="picker-footer" role="presentation">
            <button
              type="button"
              className="btn btn-ghost btn-xs"
              id={`${id}-footer`}
              onClick={() => {
                close();
                footer.onSelect();
              }}
            >
              {footer.label}
            </button>
          </li>
        )}
      </ul>
    </div>
  );
}
