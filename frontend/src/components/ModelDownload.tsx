import { useEffect, useState } from "react";
import type {
  Backend,
  ModelDownloadState,
  ModelDownloadUpdate,
  TranslationEngine,
} from "../contracts";
import { t } from "../i18n";
export function ModelDownload({
  backend,
  engine,
  report,
}: {
  backend: Backend;
  engine: TranslationEngine;
  report(error: unknown): void;
}) {
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
  const entry = models.find((model) => model.engine === engine);
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
  return (
    <>
      <div id="model-download" className="setting-row" hidden={!visible}>
        <span id="model-download-status" className="muted" role="status">
          {downloading
            ? t("modelDownloadProgress").replace("{percent}", String(percent))
            : failed
              ? t(failureKey)
              : t("modelDownloadMissing")}
        </span>
        <button
          id="model-download-action"
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
              : t("modelDownloadAction").replace(
                  "{size}",
                  ((entry?.totalBytes ?? 0) / 1e9).toFixed(1),
                )}
        </button>
      </div>
      <progress
        className="progress progress-primary"
        id="model-download-progress"
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
