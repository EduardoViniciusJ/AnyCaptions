import { useLayoutEffect, useRef, useState } from "react";
import type { AppSnapshot, Backend } from "../contracts";
import { CaptionPresenter, type CaptionFrame } from "../captions";
import { applyAppearance } from "../appearance";
import { appearanceStyle } from "./AppearanceSettings";
import { CaptionText } from "./CaptionText";
import { t } from "../i18n";
/** Streaming layout is independent of React; all visible text is rendered declaratively. */
export function CaptionView({
  backend,
  state,
  hidden,
  onError,
}: {
  backend: Backend;
  state?: AppSnapshot;
  hidden: boolean;
  onError(message: string): void;
}) {
  const report = useRef(onError);
  report.current = onError;
  const live = useRef<HTMLParagraphElement>(null);
  const presenter = useRef<CaptionPresenter>(undefined);
  const current = useRef(state);
  const [frame, setFrame] = useState<CaptionFrame>();
  current.current = state;
  useLayoutEffect(() => {
    const element = live.current!;
    const instance = new CaptionPresenter(element, undefined, setFrame);
    presenter.current = instance;
    const initial = current.current;
    if (initial) instance.setContext(initial.sessionId, initial.mode);
    let cancelled = false;
    let release: (() => void) | undefined;
    void backend
      .subscribeCaptions((update) => {
        if (
          !cancelled &&
          current.current &&
          current.current.capture !== "stopped"
        )
          instance.accept(update);
      })
      .then((unsubscribe) => {
        if (cancelled) unsubscribe();
        else release = unsubscribe;
      })
      .catch(() => {
        if (!cancelled) report.current(t("errConnect"));
      });
    const resize = () => instance.refreshLayout();
    window.addEventListener("resize", resize);
    const observer =
      typeof ResizeObserver === "undefined"
        ? undefined
        : new ResizeObserver(resize);
    observer?.observe(element);
    return () => {
      cancelled = true;
      release?.();
      observer?.disconnect();
      window.removeEventListener("resize", resize);
      instance.dispose();
      presenter.current = undefined;
    };
  }, [backend]);
  useLayoutEffect(() => {
    if (!state) return;
    applyAppearance(live.current!, state.appearance);
    presenter.current?.setContext(state.sessionId, state.mode);
    // Rótulos de falante ligados e disponíveis: um falante por linha.
    presenter.current?.setSpeakerLabels(
      state.speakerLabels && state.resources.speakerLabels,
    );
    if (state.capture === "stopped") presenter.current?.clear();
    presenter.current?.refreshLayout();
  }, [
    state?.sessionId,
    state?.mode,
    state?.capture,
    state?.appearance,
    state?.speakerLabels,
    state?.resources.speakerLabels,
    hidden,
  ]);
  return (
    <div id="preview" className="preview" hidden={hidden}>
      <p
        id="caption-preview"
        className="caption"
        hidden={!!frame?.text || state?.capture !== "stopped"}
        style={state && appearanceStyle(state.appearance)}
        data-background={state?.appearance.background ? "" : undefined}
      >
        {t("captionPlaceholder")}
      </p>
      <p
        id="caption-live"
        className="caption"
        ref={live}
        aria-live="polite"
        hidden={!frame?.text}
        data-lines={frame?.lines}
        data-state={frame?.state}
        style={{ opacity: frame?.opacity }}
      >
        <CaptionText frame={frame} />
      </p>
    </div>
  );
}
