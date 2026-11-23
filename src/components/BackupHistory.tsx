import { useMessageState } from "../i18n/use-message-state";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { backupDiff, type BackupRecord, type KeyChange } from "../api/client";
import { DiffView } from "./DiffView";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { Pagination } from "./Pagination";
import { Table, type TableColumn } from "./Table";
import { Time } from "./Time";
import { RestoreIcon } from "./icons";
import { useI18n, type MessageKey } from "../i18n";
import { tr } from "../i18n/current";
import type { TFunction } from "../i18n/context";

interface Props {
  records: BackupRecord[];
  busy: boolean;
  configurationReady: boolean;
  recoveryBackupId: string | null;
  onRestore: (backupId: string, transactionOnly: boolean) => Promise<void>;
}

/** The backup table's page size, matching the codebase's page-size convention. */
const BACKUP_PAGE_SIZE = 20;

const REASON_LABEL: Record<string, MessageKey> = {
  "switch": "backup.reason.switch",
  "provider-projection": "backup.reason.providerProjection",
  "restore-precheck": "backup.reason.restorePrecheck",
  "gateway-port-change": "backup.reason.gatewayPortChange",
  "gateway-port-rollback": "backup.reason.gatewayPortRollback",
  "client-configuration-apply": "backup.reason.clientConfigApply",
  "client-configuration-repair": "backup.reason.clientConfigRepair",
  "client-configuration-native-defaults": "backup.reason.nativeDefaults",
  "client-configuration-native-defaults-unmanaged": "backup.reason.nativeDefaultsUnmanaged",
  "client-configuration-clear-extra-configuration": "backup.reason.clearExtraConfiguration",
};

function reasonLabel(reason: string): string {
  const key = REASON_LABEL[reason];
  return key ? tr(key) : reason;
}

function isGatewayPortBackup(reason: string): boolean {
  return reason === "gateway-port-change" || reason === "gateway-port-rollback";
}

function clientLabel(app: string): string {
  return app === "codex" ? "Codex" : "Claude";
}

/** File and saved client preference differences, fetched while the region is open. */
function BackupDiff({ record }: { record: BackupRecord }) {
  const { t } = useI18n();
  const [changes, setChanges] = useState<KeyChange[] | null>(null);
  const [error, setError] = useMessageState();

  useEffect(() => {
    let active = true;
    backupDiff(record.id)
      .then((next) => {
        if (active) setChanges(next);
      })
      .catch((caught) => {
        if (active) setError(caught);
      });
    return () => {
      active = false;
    };
  }, [record]);

  if (error) return <p className="asb-warn-text">{error}</p>;
  /* Stands in for the DiffView rows, so it keeps a multi-row footprint. */
  if (changes === null) {
    return <div className="asb-skeleton asb-backup-diff-loading" aria-hidden="true" />;
  }
  if (changes.length === 0) return <p className="asb-empty">{t("backup.diff.identical")}</p>;
  return <DiffView changes={changes} label={t("backup.diff.label")} />;
}

function RestoreConfirmation({ record, transactionOnly, busy, onRestore, onClose }: {
  record: BackupRecord;
  transactionOnly: boolean;
  busy: boolean;
  onRestore: Props["onRestore"];
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [error, setError] = useMessageState();
  const restoring = useRef(false);

  async function confirm() {
    if (restoring.current) return;
    restoring.current = true;
    setError(null);
    try {
      await onRestore(record.id, transactionOnly);
      onClose();
    } catch (failure) {
      setError(failure);
    } finally {
      restoring.current = false;
    }
  }

  return (
    <ConfirmSheet
      title={t(transactionOnly ? "backup.history.confirmTransactionTitle" : "backup.history.confirmTitle")}
      confirmLabel={t("backup.confirmRestore")}
      busy={busy}
      onConfirm={() => void confirm()}
      onCancel={onClose}
    >
      <ul className="asb-dialog-details">
        <li>{t("backup.col.time")} <Time iso={record.createdAt} /></li>
        <li>{t("backup.col.client")} {clientLabel(record.app)}</li>
        <li>{t("backup.col.hash")} {record.contentHash.slice(0, 12)}</li>
        <li>{t(transactionOnly ? "backup.history.confirmTransaction" : "backup.history.confirmBackupFirst")}</li>
        <li>{t("backup.history.confirmRestoreSettings")}</li>
        <li><BackupDiff record={record} /></li>
      </ul>
      {error && <p className="asb-field-error" role="alert">{t("backup.history.restoreFailed", { reason: error })}</p>}
    </ConfirmSheet>
  );
}

function backupColumns(t: TFunction, actions: (record: BackupRecord) => ReactNode): TableColumn<BackupRecord>[] {
  return [
    { key: "createdAt", header: t("backup.col.time"), cellClassName: "asb-code",
      render: (record) => <Time iso={record.createdAt} /> },
    { key: "app", header: t("backup.col.client"), render: (record) => clientLabel(record.app) },
    { key: "reason", header: t("backup.col.reason"), render: (record) => reasonLabel(record.reason) },
    { key: "contentHash", header: t("backup.col.hash"), cellClassName: "asb-code",
      render: (record) => record.contentHash.slice(0, 12) },
    { key: "actions", header: t("backup.col.actions"), render: actions },
  ];
}

/** Backup list and the selection for an explicitly confirmed restore. */
export function BackupHistory({ records, busy, configurationReady, recoveryBackupId, onRestore }: Props) {
  const { t } = useI18n();
  const [pending, setPending] = useState<{ record: BackupRecord; transactionOnly: boolean } | null>(null);
  const [openDiffs, setOpenDiffs] = useState<ReadonlySet<string>>(() => new Set());
  const [page, setPage] = useState(1);
  const recoveryBackup = configurationReady ? undefined : records.find((record) => record.id === recoveryBackupId);
  useEffect(() => { setPage(1); }, [recoveryBackupId]);

  function toggleDiff(recordId: string) {
    setOpenDiffs((current) => {
      const next = new Set(current);
      if (next.has(recordId)) next.delete(recordId);
      else next.add(recordId);
      return next;
    });
  }

  function turnPage(next: number) {
    setPage(next);
    setOpenDiffs(new Set());
  }

  const orderedRecords = recoveryBackup
    ? [recoveryBackup, ...records.filter((record) => record.id !== recoveryBackup.id)]
    : records;
  const pageCount = Math.max(1, Math.ceil(orderedRecords.length / BACKUP_PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const pageRecords = orderedRecords.slice((currentPage - 1) * BACKUP_PAGE_SIZE, currentPage * BACKUP_PAGE_SIZE);
  const columns = backupColumns(t, (record) => {
    const transactionOnly = record.id === recoveryBackup?.id;
    return (
      <div className="asb-backup-actions">
        {isGatewayPortBackup(record.reason) ? (
          <span className="asb-scope-note">{t("backup.history.gatewayPortNote")}</span>
        ) : (
          <Button variant="secondary" disabled={busy || (!configurationReady && !transactionOnly)}
            onClick={() => setPending({ record, transactionOnly })}>
            {t(transactionOnly ? "backup.history.restoreTransaction" : "backup.history.restore")}
          </Button>
        )}
        <Button variant="secondary" disabled={busy} aria-expanded={openDiffs.has(record.id)}
          onClick={() => toggleDiff(record.id)}>
          {t("backup.history.viewDiff")}
        </Button>
      </div>
    );
  });

  return (
    <div className="asb-backups">
      {!configurationReady && <p className="asb-scope-note" role="status">
        {t(recoveryBackup ? "backup.history.transactionOnly" : "backup.history.transactionUnavailable")}
      </p>}
      {records.length === 0 ? (
        <div className="asb-empty-state">
          <span className="asb-empty-state-icon" aria-hidden="true"><RestoreIcon /></span>
          <h3 className="asb-section-title">{t("backup.history.empty")}</h3>
        </div>
      ) : <>
        <Table columns={columns} rows={pageRecords} rowKey={(record) => record.id}
          ariaLabel={t("backup.history.tableAria")}
          renderExpansion={(record) => (openDiffs.has(record.id) ? <BackupDiff record={record} /> : null)} />
        <Pagination total={records.length} page={currentPage} pageSize={BACKUP_PAGE_SIZE}
          onPageChange={turnPage} label={t("backup.history.paginationAria")} />
      </>}
      {pending && (
        <RestoreConfirmation record={pending.record} transactionOnly={pending.transactionOnly || !configurationReady}
          busy={busy} onRestore={onRestore} onClose={() => setPending(null)} />
      )}
    </div>
  );
}
