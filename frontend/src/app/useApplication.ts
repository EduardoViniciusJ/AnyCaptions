import { useEffect, useRef, useState } from "react";
import type { AppSnapshot, Backend } from "../contracts";
import { setLocale, t } from "../i18n";
import { errorMessage } from "./errors";

/** Backend snapshots are authoritative; stale events never overwrite newer commands. */
export function useApplication(backend: Backend) {
  const [state, setState] = useState<AppSnapshot>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const current = useRef<AppSnapshot>(undefined);
  const pending = useRef(false);
  const alive = useRef(false);
  const accept = (incoming: AppSnapshot) => {
    if (
      !alive.current ||
      (current.current && incoming.revision < current.current.revision)
    )
      return;
    current.current = incoming;
    setLocale(incoming.uiLanguage);
    setState(incoming);
  };
  useEffect(() => {
    alive.current = true;
    let cancelled = false;
    let release: (() => void) | undefined;
    void (async () => {
      try {
        const unsubscribe = await backend.subscribe((value) => {
          if (!cancelled) accept(value);
        });
        if (cancelled) {
          unsubscribe();
          return;
        }
        release = unsubscribe;
        const initial = await backend.getState();
        if (!cancelled) accept(initial);
      } catch {
        if (!cancelled) setError(t("errConnect"));
      }
    })();
    return () => {
      cancelled = true;
      alive.current = false;
      release?.();
    };
  }, [backend]);
  const perform = async (action: () => Promise<AppSnapshot>) => {
    if (pending.current || !current.current || !alive.current) return false;
    pending.current = true;
    setBusy(true);
    setError("");
    try {
      accept(await action());
      return true;
    } catch (failure) {
      try {
        accept(await backend.getState());
      } catch {
        /* Preserve confirmed snapshot. */
      }
      if (alive.current) setError(errorMessage(failure));
      return false;
    } finally {
      pending.current = false;
      if (alive.current) setBusy(false);
    }
  };
  const report = (failure: unknown) => {
    if (alive.current) setError(errorMessage(failure));
  };
  return { state, busy, error, perform, report, setError };
}
