import { useEffect, useRef, useState } from "react";
import { backupDiff, type BackupRecord, type KeyChange } from "../api/client";
import { DiffView } from "./DiffView";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { Table, type TableColumn } from "./Table";
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
      <Button
        variant="secondary"
        disabled={busy}
        aria-expanded={open}
        onClick={run}
      >
        查看差异
      </Button>
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

  const columns: Array<TableColumn<BackupRecord>> = [
    {
      key: "createdAt",
      header: "时间",
      cellClassName: "asb-code",
      render: (record) => <Time iso={record.createdAt} />,
    },
    { key: "app", header: "客户端", render: (record) => clientLabel(record.app) },
    { key: "reason", header: "原因", render: (record) => reasonLabel(record.reason) },
    {
      key: "contentHash",
      header: "内容哈希",
      cellClassName: "asb-code",
      render: (record) => record.contentHash.slice(0, 12),
    },
    {
      key: "actions",
      header: "操作",
      render: (record) => (
        <div className="asb-backup-actions">
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => setPending(record)}
          >
            恢复
          </Button>
          <DiffRow record={record} />
        </div>
      ),
    },
  ];

  if (records.length === 0) {
    return <p className="asb-empty">暂无备份</p>;
  }

  return (
    <div className="asb-backups">
      <Table
        columns={columns}
        rows={records}
        rowKey={(record) => record.id}
        ariaLabel="备份历史"
      />
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
