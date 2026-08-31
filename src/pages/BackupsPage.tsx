import type { BackupRecord, SwitchLog } from "../api/client";
import { BackupHistory } from "../components/BackupHistory";
import { Time } from "../components/Time";
import { clientName } from "../lib/client-name";

interface BackupsPageProps {
  records: BackupRecord[];
  busy: boolean;
  lastSwitch: SwitchLog | null;
  onRestore: (backupId: string) => void;
  onUndo: (lastSwitch: SwitchLog) => void;
  onOpenDir: () => void;
}

/** Backup history with restore, undo of the last switch, and the handoff to
 * the system file manager for cleanup. */
export function BackupsPage({
  records,
  busy,
  lastSwitch,
  onRestore,
  onUndo,
  onOpenDir,
}: BackupsPageProps) {
  return (
    <section className="asb-panel" aria-label="备份历史">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">备份历史</h2>
        <div className="asb-panel-actions">
          {lastSwitch && (
            <button
              type="button"
              className="asb-btn-danger"
              disabled={busy}
              onClick={() => onUndo(lastSwitch)}
            >
              撤回上一次切换
            </button>
          )}
          <button type="button" className="asb-btn-secondary" onClick={onOpenDir}>
            打开备份文件夹
          </button>
        </div>
      </div>
      {lastSwitch && (
        <p className="asb-scope-note">
          上次操作：{clientName(lastSwitch.app)}
          {lastSwitch.profileName
            ? ` 切换到「${lastSwitch.profileName}」`
            : " 恢复了备份"}
          ，<Time iso={lastSwitch.at} />。
        </p>
      )}
      <BackupHistory records={records} busy={busy} onRestore={onRestore} />
    </section>
  );
}
