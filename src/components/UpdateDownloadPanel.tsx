import { useId } from "react";
import { formatUpdateBytes, updateDownloadView, UPDATE_DOWNLOAD_LABELS, type UpdateDownloadSnapshot } from "../app/update-download-state";
import { useI18n } from "../i18n";
import { CommandErrorLines } from "../app/notifications";
import { Button } from "./Button";

export function UpdateDownloadPanel({ download, onCancel }: { download: UpdateDownloadSnapshot; onCancel: () => void }) {
  const { t, language } = useI18n();
  const id = useId();
  const { task, starting } = download;
  const view = updateDownloadView(download);
  const { phase, active, stale, percent } = view;
  if (!phase || phase === "installed") return null;
  const label = t(UPDATE_DOWNLOAD_LABELS[phase]);
  const bytes = formatUpdateBytes(task?.downloadedBytes ?? 0, language);
  const reading = !task ? label : task.totalBytes && task.totalBytes > 0
    ? `${bytes} / ${formatUpdateBytes(task.totalBytes, language)}` : t("backup.update.downloadedUnknown", { size: bytes });
  const size = stale ? t("update.lastReading", { size: reading }) : reading;
  const heading = stale && task ? t("update.lastPhase", { phase: t(UPDATE_DOWNLOAD_LABELS[task.phase]) }) : label;
  const progress = percent === null ? null : stale ? t("update.lastProgress", { percent }) : `${percent}%`;
  return <div className="asb-update-download" data-phase={phase}>
    <div className="asb-update-download-heading">
      <span id={id} className="asb-checkbox-label" role="status">{heading}</span>
      {progress && <span className="asb-update-download-percent asb-num" aria-hidden="true">{progress}</span>}
    </div>
    {active && <div className="asb-update-download-meter" role="progressbar" aria-labelledby={id}
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={stale ? undefined : percent ?? undefined}
      aria-valuetext={`${heading} · ${size}`} data-indeterminate={percent === null} data-stale={stale}>
      <span style={percent === null ? undefined : { width: `${percent}%` }} />
    </div>}
    <div className="asb-update-download-footer">
      <span className="asb-app-setting-detail asb-num">{active ? size : t("backup.update.retryHint")}</span>
      {view.canCancel && <Button variant="secondary" size="compact" autoFocus={starting}
        disabled={phase === "cancelling"} aria-busy={phase === "cancelling"} onClick={onCancel}>
        {t(phase === "cancelling" ? "backup.update.cancelling"
          : view.cancelError != null ? "backup.update.retryCancel" : "backup.update.cancelDownload")}
      </Button>}
    </div>
    {phase === "installing" && <p className="asb-app-setting-detail">{t("backup.update.installHint")}</p>}
    {view.error != null && <div className="asb-fail-text" role="alert"><CommandErrorLines error={view.error} /></div>}
    {view.cancelError != null && <div className="asb-warn-text" role="alert">
      <div>{t("update.cancelFailed")}</div><CommandErrorLines error={view.cancelError} />
    </div>}
    {view.readError != null && <div className="asb-warn-text" role="alert">
      <div>{t("backup.update.statusRetry")}</div><CommandErrorLines error={view.readError} />
    </div>}
  </div>;
}
