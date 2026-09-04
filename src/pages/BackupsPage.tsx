import { useState } from "react";
import type { BackupRecord, ConfigWriteRecord } from "../api/client";
import type { useCloudBackup } from "../app/useCloudBackup";
import { BackupHistory } from "../components/BackupHistory";
import { Button } from "../components/Button";
import { CloudBackupPanel } from "../components/CloudBackupPanel";
import { Time } from "../components/Time";
import { clientName } from "../lib/client-name";

interface BackupsPageProps {
  records: BackupRecord[];
  busy: boolean;
  lastSwitch: ConfigWriteRecord | null;
  cloudBackup: ReturnType<typeof useCloudBackup>;
  onRestore: (backupId: string) => void;
  onUndo: (lastSwitch: ConfigWriteRecord) => void;
  onOpenDir: () => void;
}

type BackupTab = "local" | "cloud";

/** Backup history with restore, undo of the last switch, and the handoff to
 * the system file manager for cleanup. */
export function BackupsPage({
  records,
  busy,
  lastSwitch,
  cloudBackup,
  onRestore,
  onUndo,
  onOpenDir,
}: BackupsPageProps) {
  const [activeTab, setActiveTab] = useState<BackupTab>("local");

  return (
    <section className="asb-panel" aria-label="备份">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">备份</h2>
        <div className="asb-tabs" role="tablist" aria-label="备份类型">
          <button
            id="backup-local-tab"
            type="button"
            role="tab"
            aria-selected={activeTab === "local"}
            aria-controls="backup-local-panel"
            className={`asb-tab${activeTab === "local" ? " is-on" : ""}`}
            onClick={() => setActiveTab("local")}
          >
            本地备份
          </button>
          <button
            id="backup-cloud-tab"
            type="button"
            role="tab"
            aria-selected={activeTab === "cloud"}
            aria-controls="backup-cloud-panel"
            className={`asb-tab${activeTab === "cloud" ? " is-on" : ""}`}
            onClick={() => setActiveTab("cloud")}
          >
            加密云端备份
          </button>
        </div>
      </div>
      {activeTab === "local" ? (
        <div
          id="backup-local-panel"
          className="asb-backup-local"
          role="tabpanel"
          aria-labelledby="backup-local-tab"
        >
          <div className="asb-backup-toolbar">
            <div className="asb-panel-actions">
              {lastSwitch && (
                <Button
                  variant="danger"
                  disabled={busy}
                  onClick={() => onUndo(lastSwitch)}
                >
                  撤回上一次切换
                </Button>
              )}
              <Button variant="secondary" onClick={onOpenDir}>
                打开备份文件夹
              </Button>
            </div>
          </div>
          {lastSwitch && (
            <p className="asb-scope-note">
              上次操作：{clientName(lastSwitch.app)}
              {lastSwitch.operation === "projection" && lastSwitch.profileName
                ? ` 已投影供应商「${lastSwitch.profileName}」`
                : lastSwitch.operation === "restore"
                  ? " 恢复了备份"
                  : " 已应用通用设置投影"}
              ，<Time iso={lastSwitch.at} />。
            </p>
          )}
          <BackupHistory records={records} busy={busy} onRestore={onRestore} />
        </div>
      ) : (
        <div
          id="backup-cloud-panel"
          role="tabpanel"
          aria-labelledby="backup-cloud-tab"
        >
          <CloudBackupPanel
            settings={cloudBackup.settings}
            loaded={cloudBackup.loaded}
            busy={busy}
            onSave={cloudBackup.saveSettings}
            onTestConnection={cloudBackup.testConnection}
            onUpload={cloudBackup.upload}
            onRestore={cloudBackup.restore}
          />
        </div>
      )}
    </section>
  );
}
