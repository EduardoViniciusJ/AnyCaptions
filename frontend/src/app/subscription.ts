/**
 * Inscrição num evento do backend pelo tempo de vida de um efeito do React.
 *
 * `listen()` do Tauri é assíncrono: o componente pode desmontar antes de a
 * inscrição existir. Esta função cuida das três situações que cada tela
 * repetia à mão: eventos que chegam depois do desmonte são ignorados, uma
 * inscrição que só termina depois do desmonte é desfeita na hora, e a falha
 * ao inscrever só é avisada se o componente ainda estiver montado.
 *
 * Devolve a função de limpeza do efeito.
 */
export function attach<T>(
  subscribe: (handler: (event: T) => void) => Promise<() => void>,
  handler: (event: T) => void,
  options: {
    /** Roda uma vez, com a inscrição já ativa (ex.: buscar o estado inicial). */
    ready?: (active: () => boolean) => void | Promise<void>;
    /** A inscrição (ou `ready`) falhou e o componente continua montado. */
    failed?: () => void;
  } = {},
): () => void {
  let cancelled = false;
  let release: (() => void) | undefined;
  const active = () => !cancelled;
  void (async () => {
    try {
      const unsubscribe = await subscribe((event) => {
        if (!cancelled) handler(event);
      });
      if (cancelled) {
        unsubscribe();
        return;
      }
      release = unsubscribe;
      await options.ready?.(active);
    } catch {
      if (!cancelled) options.failed?.();
    }
  })();
  return () => {
    cancelled = true;
    release?.();
  };
}
