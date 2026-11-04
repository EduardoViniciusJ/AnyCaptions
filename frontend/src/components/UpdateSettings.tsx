import { useState } from "react";
import type { AppSnapshot, Backend, UpdateCheck } from "../contracts";
import { t } from "../i18n";
import { Field } from "./Field";
import { UpdateAction } from "./UpdateAction";

type Status =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "result"; check: UpdateCheck }
  | { kind: "failed" };

/**
 * Seção "Atualizações", no mesmo padrão das outras linhas: a versão com o
 * botão "Verificar agora", a chave de verificar ao abrir, e uma linha de
 * oferta (texto + botão) quando há versão nova, como a do download de
 * modelos. A consulta vai só à página de versões do projeto no GitHub e nada
 * é instalado sem o clique em "Atualizar".
 */
export function UpdateSettings({
  backend,
  state,
  perform,
  report,
}: {
  backend: Backend;
  state: AppSnapshot;
  perform(action: () => Promise<AppSnapshot>): Promise<boolean>;
  report(error: unknown): void;
}) {
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const check = async () => {
    setStatus({ kind: "checking" });
    try {
      const result = await backend.checkForUpdates();
      setStatus({ kind: "result", check: result });
    } catch {
      setStatus({ kind: "failed" });
    }
  };
  const available = status.kind === "result" && status.check.available;
  const message = (() => {
    switch (status.kind) {
      case "checking":
        return t("updateChecking");
      case "result":
        return available
          ? t("updateAvailable").replace("{version}", status.check.latest)
          : t("updateLatest").replace("{version}", status.check.current);
      case "failed":
        return t("updateFailed");
      default:
        return t("updateHint");
    }
  })();
  return (
    <>
      <h3 className="settings-subtitle">{t("updates")}</h3>
      <Field id="update-check-now" label={t("appVersion")}>
        <div className="version-row">
          <span id="app-version">v{state.version}</span>
          <button
            className="btn btn-sm"
            id="update-check-now"
            type="button"
            disabled={status.kind === "checking"}
            onClick={() => void check()}
          >
            {t("updateCheckNow")}
          </button>
        </div>
      </Field>
      <Field id="update-check" label={t("updateCheckOnStart")}>
        <input
          className="toggle toggle-sm toggle-primary"
          id="update-check"
          type="checkbox"
          checked={state.updateCheck}
          onChange={(event) => {
            const enabled = event.target.checked;
            if (enabled !== state.updateCheck)
              void perform(() => backend.setUpdateCheck(enabled));
          }}
        />
      </Field>
      <div className={`setting-row${available ? " model-download" : " update-status-row"}`}>
        <p id="update-status" className="muted" role="status">
          {message}
        </p>
        {available && status.kind === "result" && (
          <UpdateAction backend={backend} check={status.check} id="update-download" report={report} />
        )}
      </div>
    </>
  );
}
