import { useEffect, useState } from "react";
import type {
  Backend,
  ModelDownloadState,
  ModelDownloadUpdate,
  ModelKind,
  TranslationEngine,
} from "../contracts";
import { t, type MessageKey } from "../i18n";

export interface ModelDownloads {
  models: ModelDownloadState[];
  update: ModelDownloadUpdate | null;
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
    let cancelled = false;
    let release: (() => void) | undefined;
    const refresh = async () => {
      try {
        const result = await backend.getModelDownloads();
        if (!cancelled) setModels(result);
      } catch {
        /* No offer without an authoritative catalog. */
      }
    };
    void (async () => {
      try {
        const unsubscribe = await backend.subscribeModelDownloads((event) => {
          if (cancelled) return;
          setUpdate(event);
          if (event.phase === "ready") void refresh();
        });
        if (cancelled) {
          unsubscribe();
          return;
        }
        release = unsubscribe;
        await refresh();
      } catch {
        /* Optional download status; capture is independent. */
      }
    })();
    return () => {
      cancelled = true;
      release?.();
    };
  }, [backend]);
  return { models, update, setUpdate };
}

/**
 * Oferta de download de um modelo do catálogo: pelo mecanismo de tradução que
 * ele serve (`engine`) ou pelo tipo (`kind`, ex.: diarização). A interface só
 * passa o id do catálogo; o backend resolve e valida a origem.
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
  const { models, update, setUpdate } = downloads;
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
  // Tamanhos abaixo de 1 GB em MB, para não mostrar "0.1 GB".
  const sizeLabel =
    bytes >= 1e9
      ? t("modelDownloadAction").replace("{size}", (bytes / 1e9).toFixed(1))
      : t("modelDownloadActionMb").replace(
          "{size}",
          String(Math.round(bytes / 1e6)),
        );
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
    </>
  );
}
