import { useEffect, useState } from "react";
import type {
  Backend,
  ModelDownloadState,
  ModelDownloadUpdate,
  ModelKind,
  TranslationEngine,
} from "../contracts";
import { t, type MessageKey } from "../i18n";
import { attach } from "../app/subscription";

export interface ModelDownloads {
  models: ModelDownloadState[];
  update: ModelDownloadUpdate | null;
  setModels(models: ModelDownloadState[]): void;
  setUpdate(update: ModelDownloadUpdate | null): void;
}

/**
 * Uma única inscrição nos downloads do catálogo, compartilhada por todas as
 * linhas de download do painel (tradução e diarização).
 */
export function useModelDownloads(backend: Backend): ModelDownloads {
  const [models, setModels] = useState<ModelDownloadState[]>([]);
  const [update, setUpdate] = useState<ModelDownloadUpdate | null>(null);
  useEffect(() => {
    let mounted = true;
    const refresh = async () => {
      try {
        const result = await backend.getModelDownloads();
        if (mounted) setModels(result);
      } catch {
        /* No offer without an authoritative catalog. */
      }
    };
    // A failed subscription is silent: the download status is optional and
    // capture does not depend on it.
    const detach = attach(
      backend.subscribeModelDownloads.bind(backend),
      (event) => {
        setUpdate(event);
        if (event.phase === "ready") void refresh();
      },
      { ready: refresh },
    );
    return () => {
      mounted = false;
      detach();
    };
  }, [backend]);
  return { models, update, setModels, setUpdate };
}

/** "2.8 GB" or "491 MB": sizes under 1 GB in MB, so nothing reads "0.5 GB". */
export function modelSize(bytes: number): string {
  return bytes >= 1e9
    ? t("sizeGb").replace("{size}", (bytes / 1e9).toFixed(1))
    : t("sizeMb").replace("{size}", String(Math.round(bytes / 1e6)));
}

/**
 * Um modelo do catálogo que o usuário baixa e remove: pelo mecanismo de
 * tradução que ele serve (`engine`) ou pelo tipo (`kind`, ex.: diarização).
 * Sem o modelo, oferece o download (com progresso, cancelar e tentar de
 * novo); com ele, mostra o espaço ocupado e a remoção, confirmada na própria
 * linha. A interface só passa o id do catálogo; o backend resolve a origem e
 * decide o que pode ser apagado.
 */
export function ModelDownload({
  backend,
  downloads,
  engine,
  kind,
  idPrefix = "model-download",
  missingKey = "modelDownloadMissing",
  report,
}: {
  backend: Backend;
  downloads: ModelDownloads;
  engine?: TranslationEngine;
  kind?: ModelKind;
  idPrefix?: string;
  missingKey?: MessageKey;
  report(error: unknown): void;
}) {
  const { models, update, setModels, setUpdate } = downloads;
  const [confirming, setConfirming] = useState(false);
  const [removing, setRemoving] = useState(false);
  const entry = kind
    ? models.find((model) => model.kind === kind)
    : models.find((model) => model.engine === engine);
  const event = update?.model === entry?.model ? update : null;
  const downloading = event
    ? event.phase === "downloading"
    : entry?.downloading;
  const total = event?.totalBytes ?? entry?.totalBytes ?? 0;
  const percent = total
    ? Math.min(100, Math.floor(((event?.receivedBytes ?? 0) / total) * 100))
    : 0;
  const failed = event?.phase === "failed" && event.reason !== "cancelled";
  const failureKey =
    event?.reason === "disk"
      ? "modelDownloadFailedDisk"
      : event?.reason === "corrupted"
        ? "modelDownloadFailedCorrupted"
        : "modelDownloadFailedNetwork";
  const visible =
    !!entry?.downloadable && !entry.installed && event?.phase !== "ready";
  const bytes = entry?.totalBytes ?? 0;
  const size = modelSize(bytes);
  const sizeLabel = t(
    bytes >= 1e9 ? "modelDownloadAction" : "modelDownloadActionMb",
  ).replace("{size}", bytes >= 1e9 ? (bytes / 1e9).toFixed(1) : String(Math.round(bytes / 1e6)));
  const remove = async () => {
    if (!entry || removing) return;
    setRemoving(true);
    try {
      const result = await backend.removeModel(entry.model);
      // A finished download of this model must not keep hiding the offer.
      setUpdate(null);
      setModels(result);
    } catch (error) {
      report(error);
    } finally {
      setRemoving(false);
      setConfirming(false);
    }
  };
  return (
    <>
      <div
        id={idPrefix}
        className="setting-row model-download"
        hidden={!visible}
      >
        <span id={`${idPrefix}-status`} className="muted" role="status">
          {downloading
            ? t("modelDownloadProgress").replace("{percent}", String(percent))
            : failed
              ? t(failureKey)
              : t(missingKey)}
        </span>
        <button
          id={`${idPrefix}-action`}
          className="btn btn-sm"
          type="button"
          onClick={() => {
            if (!entry) return;
            if (downloading) {
              void backend.cancelModelDownload().catch(report);
              return;
            }
            setUpdate({
              model: entry.model,
              phase: "downloading",
              receivedBytes: 0,
              totalBytes: entry.totalBytes,
              reason: null,
            });
            void backend.downloadModel(entry.model).catch(() =>
              setUpdate({
                model: entry.model,
                phase: "failed",
                receivedBytes: 0,
                totalBytes: null,
                reason: "network",
              }),
            );
          }}
        >
          {downloading
            ? t("modelDownloadCancel")
            : failed
              ? t("modelDownloadRetry")
              : sizeLabel}
        </button>
      </div>
      <progress
        className="progress progress-primary model-download-progress"
        id={`${idPrefix}-progress`}
        aria-label={t("modelDownloadProgress").replace(
          "{percent}",
          String(percent),
        )}
        max={100}
        value={percent}
        hidden={!visible || !downloading}
      />
      <div
        id={`${idPrefix}-installed`}
        className="setting-row model-download"
        hidden={!entry?.installed}
      >
        <span id={`${idPrefix}-installed-status`} className="muted" role="status">
          {removing
            ? t("modelRemoving")
            : confirming
              ? t("modelRemoveConfirm").replace("{size}", size)
              : t("modelInstalled").replace("{size}", size)}
        </span>
        <span className="setting-actions">
          {confirming && (
            <button
              id={`${idPrefix}-remove-cancel`}
              className="btn btn-sm"
              type="button"
              disabled={removing}
              onClick={() => setConfirming(false)}
            >
              {t("modelRemoveCancel")}
            </button>
          )}
          <button
            id={`${idPrefix}-remove`}
            className={`btn btn-sm${confirming ? " btn-error" : " btn-ghost"}`}
            type="button"
            disabled={removing}
            onClick={() => (confirming ? void remove() : setConfirming(true))}
          >
            {t("modelRemove")}
          </button>
        </span>
      </div>
    </>
  );
}
