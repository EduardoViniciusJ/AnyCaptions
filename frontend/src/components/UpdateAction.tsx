import { useEffect, useState } from "react";
import type { Backend, UpdateCheck, UpdateProgress } from "../contracts";
import { t } from "../i18n";

/**
 * Ação de uma versão nova: "Atualizar" baixa e instala pelo próprio app, com o
 * progresso no lugar do botão. Sem instalação possível, ou depois de uma
 * falha, o botão abre a página de versões no navegador.
 */
export function UpdateAction({
  backend,
  check,
  id,
  compact = false,
  report,
}: {
  backend: Backend;
  check: UpdateCheck;
  id: string;
  compact?: boolean;
  report(error: unknown): void;
}) {
  const [progress, setProgress] = useState<UpdateProgress>();
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!check.installable) return;
    let cancelled = false;
    let release: (() => void) | undefined;
    void backend
      .subscribeUpdateProgress((event) => {
        if (cancelled) return;
        if (event.phase === "failed") {
          setProgress(undefined);
          setFailed(true);
        } else {
          setFailed(false);
          setProgress(event);
        }
      })
      .then((unsubscribe) => {
        if (cancelled) unsubscribe();
        else release = unsubscribe;
      })
      .catch(() => {
        /* Sem progresso a mostrar; o botão de baixar continua disponível. */
      });
    return () => {
      cancelled = true;
      release?.();
    };
  }, [backend, check.installable]);

  const size = compact ? "btn-xs" : "btn-sm";
  if (progress) {
    const percent = Math.floor(
      (progress.receivedBytes * 100) / Math.max(progress.totalBytes, 1),
    );
    return (
      <span id={`${id}-progress`} className="muted" role="status">
        {progress.phase === "downloading"
          ? t("updateDownloading").replace("{percent}", String(percent))
          : t("updateInstalling")}
      </span>
    );
  }
  const openPage = () => void backend.openReleasesPage().catch(report);
  if (!check.installable || failed) {
    return (
      <>
        {failed && (
          <span id={`${id}-failed`} className="muted" role="alert">
            {t("updateInstallFailed")}
          </span>
        )}
        <button className={`btn ${size} btn-primary`} id={id} type="button" onClick={openPage}>
          {t("updateDownload")}
        </button>
      </>
    );
  }
  return (
    <button
      className={`btn ${size} btn-primary`}
      id={id}
      type="button"
      onClick={() => {
        setFailed(false);
        // O primeiro evento troca o botão pelo progresso.
        setProgress({ phase: "downloading", receivedBytes: 0, totalBytes: 0 });
        backend.installUpdate().catch((error: unknown) => {
          setProgress(undefined);
          setFailed(true);
          report(error);
        });
      }}
    >
      {t("updateInstall")}
    </button>
  );
}
