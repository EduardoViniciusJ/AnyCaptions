import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { t } from "../i18n";
import type { Backend } from "../contracts";
import {
  DictationIndicator as Indicator,
  type DictationFrame,
} from "../dictation";
export function DictationIndicator({
  backend,
  onError,
  feedbackOnly = false,
}: {
  backend: Pick<Backend, "subscribeDictation">;
  feedbackOnly?: boolean;
  onError(message: string): void;
}) {
  const [frame, setFrame] = useState<DictationFrame>();
  const text = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    if (text.current) text.current.scrollTop = text.current.scrollHeight;
  }, [frame?.text]);
  const report = useRef(onError);
  report.current = onError;
  useEffect(() => {
    const indicator = new Indicator(setFrame, (message) =>
      report.current(message),
    );
    let cancelled = false;
    let release: (() => void) | undefined;
    void backend
      .subscribeDictation((event) => {
        if (!cancelled) indicator.accept(event);
      })
      .then((unsubscribe) => {
        if (cancelled) unsubscribe();
        else release = unsubscribe;
      })
      .catch(() => {
        if (!cancelled) report.current(t("errConnect"));
      });
    return () => {
      cancelled = true;
      release?.();
      indicator.dispose();
    };
  }, [backend]);
  if (feedbackOnly) return null;
  return (
    <div
      id="dictation-indicator"
      className="dictation-indicator"
      hidden={frame?.hidden ?? true}
      data-phase={frame?.phase}
      data-compact={String(frame?.compact ?? false)}
    >
      <span className="dictation-dot" aria-hidden="true" />
      <span
        id="dictation-text"
        className="dictation-text"
        role="status"
        ref={text}
        title={frame?.hint}
      >
        {frame?.text}
      </span>
      <span id="dictation-elapsed" className="dictation-elapsed">
        {frame?.elapsed}
      </span>
    </div>
  );
}
