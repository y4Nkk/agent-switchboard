import { useEffect, useRef, useState } from "react";
import { backupDiff, type BackupRecord, type KeyChange } from "../api/client";
import { DiffView } from "./DiffView";
import { ConfirmSheet } from "./ConfirmSheet";
import { Time } from "./Time";

interface Props {
  records: BackupRecord[];
  busy: boolean;
  onRestore: (backupId: string) => void;
}

function reasonLabel(reason: string): string {
  if (reason === "switch") return "切换前备份";
  if (reason === "restore-precheck") return "恢复前备份";
  return reason;
}

function clientLabel(app: string): string {
  return app === "codex" ? "Codex" : "Claude";
}

/** One backup's owned-key difference against the live file. */
function DiffRow({ record }: { record: BackupRecord }) {
  const [changes, setChanges] = useState<KeyChange[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState(false);
  const requestVersion = useRef(0);

  useEffect(() => {
    requestVersion.current += 1;
    setChanges(null);
    setError(null);
    setBusy(false);
    setOpen(false);
  }, [record]);

  const run = async () => {
    if (busy) return;
    if (open) {
      setOpen(false);
      return;
    }
    setOpen(true);
    if (changes) return;
    const version = requestVersion.current;
    setBusy(true);
    setError(null);
    try {
      const nextChanges = await backupDiff(record.id);
      if (requestVersion.current === version) setChanges(nextChanges);
    } catch (caught) {
      if (requestVersion.current === version) {
        setError((caught as { message?: string }).message ?? "无法生成差异");
      }
    } finally {
      if (requestVersion.current === version) setBusy(false);
    }
  };

  return (
    <div className="asb-backup-diff">
      <button
        type="button"
        className="asb-btn-secondary"
        disabled={busy}
        aria-expanded={open}
        onClick={run}
      >
        查看差异
      </button>
      {open && error && <p className="asb-warn-text">{error}</p>}
      {open && changes !== null && (changes.length > 0 ? (
        <DiffView changes={changes} label="当前文件与备份的差异" />
      ) : (
        <p className="asb-empty">与当前文件一致</p>
      ))}
    </div>
  );
}

/** Recent validation and restore history (DESIGN.md §7 bottom band). */
export function BackupHistory({ records, busy, onRestore }: Props) {
  const [pending, setPending] = useState<BackupRecord | null>(null);

  if (records.length === 0) {
    return <p className="asb-empty">暂无备份</p>;
  }

  return (
    <div className="asb-backups">
      <table className="asb-table">
        <thead>
          <tr>
            <th scope="col">时间</th>
            <th scope="col">客户端</th>
            <th scope="col">原因</th>
            <th scope="col">内容哈希</th>
            <th scope="col">操作</th>
          </tr>
        </thead>
        <tbody>
          {records.map((record) => (
            <tr key={record.id}>
              <td className="asb-code">
                <Time iso={record.createdAt} />
              </td>
              <td>{clientLabel(record.app)}</td>
              <td>{reasonLabel(record.reason)}</td>
              <td className="asb-code">{record.contentHash.slice(0, 12)}</td>
              <td>
                <div className="asb-backup-actions">
                  <button
                    type="button"
                    className="asb-btn-secondary"
                    disabled={busy}
                    onClick={() => setPending(record)}
                  >
                    恢复
                  </button>
                  <DiffRow record={record} />
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {pending && (
        <ConfirmSheet
          title="恢复备份"
          details={[
            <>
              时间 <Time iso={pending.createdAt} />
            </>,
            `客户端 ${clientLabel(pending.app)}`,
            `内容哈希 ${pending.contentHash.slice(0, 12)}`,
            "当前内容会先另行备份，恢复本身可撤销。",
          ]}
          confirmLabel="确认恢复"
          onConfirm={() => {
            onRestore(pending.id);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
