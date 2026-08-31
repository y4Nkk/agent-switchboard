import { openUrl } from "@tauri-apps/plugin-opener";
import type { ReactNode } from "react";
import type { UpdateCheck } from "../api/client";
import { Time } from "./Time";

/** Manual software-update check on the settings page. Pure display: the
 * check itself (busy gate + error toast) is owned by App, exactly like the
 * endpoint probe in the provider editor. */
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
    : "检查新版本";
  const detail: ReactNode = result ? (
    <>
      当前版本 {result.currentVersion} · 检查于 <Time iso={result.checkedAt} />
    </>
  ) : (
    "从 GitHub 发布页检查 Agent Switchboard 的最新版本"
  );
  return (
    <section className="asb-app-settings-group" aria-labelledby="software-update">
      <h3 id="software-update" className="asb-toggle-group-title">
        软件更新
      </h3>
      <div className="asb-app-setting-row">
        <div className="asb-app-setting-copy">
          <span className="asb-checkbox-label">{label}</span>
          <span className="asb-app-setting-detail">{detail}</span>
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
