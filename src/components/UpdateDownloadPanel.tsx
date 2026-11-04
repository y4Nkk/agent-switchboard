import { useId } from "react";
import { downloadPercent, type UpdateDownloadView } from "../app/update-download-state";
import { useI18n, type MessageKey } from "../i18n";
import { errorText } from "../i18n/errors";
import { Button } from "./Button";

const PHASE_LABEL = {
  downloading: "backup.update.downloading", verifying: "backup.update.verifying",
  cancelling: "backup.update.cancelling", installing: "backup.update.installing",
  installed: "backup.update.restartRequired", cancelled: "backup.update.cancelled", failed: "backup.update.failed",
} as const satisfies Record<string, MessageKey>;

function formatBytes(bytes: number, language: string): string {
  const units = ["B", "KB", "MB", "GB"];
  const unit = bytes > 0 ? Math.min(3, Math.floor(Math.log10(bytes) / 3)) : 0;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(bytes / 1000 ** unit)} ${units[unit]}`;
}

export function UpdateDownloadPanel({ download, busy, onCancel, onRetry }: {
  download: UpdateDownloadView; busy: boolean; onCancel: () => void; onRetry: () => void;
}) {
  const { t, language } = useI18n();
  const id = useId();
  const { task, starting, cancelling, readError } = download;
  if (!starting && (!task || task.phase === "installed")) return null;
  const phase = cancelling && (!task || task.phase === "downloading" || task.phase === "verifying")
    ? "cancelling" : task?.phase ?? "downloading";
  const working = starting || !["cancelled", "failed"].includes(phase);
  const canCancel = starting || phase === "downloading" || phase === "verifying" || phase === "cancelling";
  const percent = task && phase === "downloading" ? downloadPercent(task) : null;
  const label = t(PHASE_LABEL[phase]);
  const bytes = formatBytes(task?.downloadedBytes ?? 0, language);
  const size = task?.totalBytes ? `${bytes} / ${formatBytes(task.totalBytes, language)}`
    : t("backup.update.downloadedUnknown", { size: bytes });
  return <div className="asb-update-download" data-phase={phase}>
    <div className="asb-update-download-heading">
      <span id={id} className="asb-checkbox-label" role="status">{label}</span>
      {percent !== null && <span className="asb-update-download-percent asb-num" aria-hidden="true">{percent}%</span>}
    </div>
    {working && <div className="asb-update-download-meter" role="progressbar" aria-labelledby={id}
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}
      aria-valuetext={`${label} · ${size}`} data-indeterminate={percent === null}>
      <span style={percent === null ? undefined : { width: `${percent}%` }} />
    </div>}
    <div className="asb-update-download-footer">
      <span className="asb-app-setting-detail asb-num">{working ? size : t("backup.update.retryHint")}</span>
      {(canCancel || !working) && <Button variant={working ? "secondary" : "primary"} size="compact"
        autoFocus={starting} disabled={phase === "cancelling" || (!working && busy)}
        aria-busy={phase === "cancelling"} onClick={working ? onCancel : onRetry}>
        {t(!working ? "backup.update.retryDownload"
          : phase === "cancelling" ? "backup.update.cancelling" : "backup.update.cancelDownload")}
      </Button>}
    </div>
    {phase === "installing" && <p className="asb-app-setting-detail">{t("backup.update.installHint")}</p>}
    {task?.error && <p className="asb-fail-text" role="alert">{errorText(task.error, t)}</p>}
    {readError && <p className="asb-warn-text" role="alert">{t("backup.update.statusRetry")} {readError}</p>}
  </div>;
}
