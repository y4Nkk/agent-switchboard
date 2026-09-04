import type { ReactNode } from "react";
import type { UpdateCheck } from "../api/client";
import type { UpdateDownloadProgress } from "../app/useUpdateCheck";
import { Button } from "./Button";
import { Time } from "./Time";

function formatProgress(progress: UpdateDownloadProgress): string {
  if (progress.totalBytes === null || progress.totalBytes === 0) {
    return `正在下载更新 ${Math.floor(progress.downloadedBytes / 1024)} KB`;
  }
  return `正在下载更新 ${Math.min(100, Math.floor((progress.downloadedBytes / progress.totalBytes) * 100))}%`;
}

/** Software-update state on the settings page. Startup checks are silent;
 * download, verification and installation are explicit user actions. */
export function UpdateSection({
  result,
  busy,
  installing,
  progress,
  checkedAt,
  restartRequired,
  onCheck,
  onInstall,
  onRestart,
}: {
  result: UpdateCheck | null;
  busy: boolean;
  installing: boolean;
  progress: UpdateDownloadProgress | null;
  checkedAt: string | null;
  restartRequired: boolean;
  onCheck: () => void;
  onInstall: () => void;
  onRestart: () => void;
}) {
  const label = restartRequired
    ? installing
      ? "正在重新启动"
      : "更新已安装，需要重启"
    : result
    ? installing
      ? progress
        ? formatProgress(progress)
        : "正在安装更新"
      : `发现新版本 ${result.latestVersion}`
    : busy
      ? "正在检查新版本"
      : checkedAt
        ? "已是最新版本"
      : "检查新版本";
  const detail: ReactNode = checkedAt ? (
    <>
      {result ? `当前版本 ${result.currentVersion} · ` : null}检查于 <Time iso={checkedAt} />
    </>
  ) : null;
  return (
    <section className="asb-app-settings-group" aria-labelledby="software-update">
      <h3 id="software-update" className="asb-toggle-group-title">
        软件更新
      </h3>
      <div className="asb-app-setting-row">
        <div className="asb-app-setting-copy">
          <span className="asb-checkbox-label">{label}</span>
          {detail ? <span className="asb-app-setting-detail">{detail}</span> : null}
        </div>
        <div className="asb-panel-actions">
          {restartRequired ? (
            <Button variant="primary" disabled={installing} onClick={onRestart}>
              重新启动
            </Button>
          ) : result ? (
            <Button
              variant="primary"
              disabled={busy || installing}
              onClick={onInstall}
            >
              下载并安装
            </Button>
          ) : null}
          <Button variant="secondary" disabled={busy || installing || restartRequired} onClick={onCheck}>
            检查更新
          </Button>
        </div>
      </div>
    </section>
  );
}
