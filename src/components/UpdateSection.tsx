import { openUrl } from "@tauri-apps/plugin-opener";
import type { ReactNode } from "react";
import type { UpdateCheck } from "../api/client";
import { Time } from "./Time";

/** Software-update state on the settings page. The startup lookup is silent;
 * a user-triggered retry exposes failures through the application error UI. */
export function UpdateSection({
  result,
  busy,
  onCheck,
}: {
  result: UpdateCheck | null;
  busy: boolean;
  onCheck: () => void;
}) {
  const label = result
    ? result.updateAvailable
      ? `发现新版本 ${result.latestVersion}`
      : "已是最新版本"
    : busy
      ? "正在检查新版本"
      : "检查新版本";
  const detail: ReactNode = result ? (
    <>当前版本 {result.currentVersion} · 检查于 <Time iso={result.checkedAt} /></>
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
          {result?.updateAvailable === true && (
            <button
              type="button"
              className="asb-btn-primary"
              onClick={() => void openUrl(result.releaseUrl)}
            >
              打开下载页面
            </button>
          )}
          <button type="button" className="asb-btn-secondary" disabled={busy} onClick={onCheck}>
            检查更新
          </button>
        </div>
      </div>
    </section>
  );
}
