import { useEffect, useRef, useState } from "react";
import type { AppSnapshot, Backend, UpdateCheck } from "../contracts";
import { t } from "../i18n";
import { UpdateAction } from "./UpdateAction";

/**
 * Aviso de nova versão ao abrir o app, só quando o usuário ligou a
 * verificação nas configurações. Consulta uma vez por execução; falhas são
 * silenciosas aqui (o botão "Verificar agora" das configurações explica).
 */
export function UpdateNotice({
  backend,
  state,
  report,
}: {
  backend: Backend;
  state?: AppSnapshot;
  report(error: unknown): void;
}) {
  const [check, setCheck] = useState<UpdateCheck>();
  const [dismissed, setDismissed] = useState(false);
  const started = useRef(false);
  const enabled = state?.updateCheck === true;
  useEffect(() => {
    if (!enabled || started.current) return;
    started.current = true;
    let cancelled = false;
    void backend
      .checkForUpdates()
      .then((result) => {
        if (!cancelled) setCheck(result);
      })
      .catch(() => {
        /* Sem rede ou GitHub fora: nada a avisar. */
      });
    return () => {
      cancelled = true;
    };
  }, [backend, enabled]);
  if (!check?.available || dismissed) return null;
  return (
    <p id="update-notice" className="notice" role="status">
      <span>{t("updateAvailable").replace("{version}", check.latest)}</span>
      <UpdateAction backend={backend} check={check} id="update-notice-download" compact report={report} />
      <button
        className="btn btn-xs btn-ghost"
        id="update-notice-dismiss"
        type="button"
        aria-label="×"
        onClick={() => setDismissed(true)}
      >
        ×
      </button>
    </p>
  );
}
