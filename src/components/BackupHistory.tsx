import { useMessageState } from "../i18n/use-message-state";
import { useEffect, useId, useRef, useState } from "react";
import { backupDiff, type BackupRecord, type KeyChange } from "../api/client";
import { Button } from "./Button";
import { ClientLogo } from "./ClientLogo";
import { Collapsible, CollapsibleTrigger } from "./Collapsible";
import { ConfirmSheet } from "./ConfirmSheet";
import { DiffView } from "./DiffView";
import { Pagination } from "./Pagination";
import { Time } from "./Time";
import { useI18n, type MessageKey } from "../i18n";
import { tr } from "../i18n/current";

interface Props {
  records: BackupRecord[];
  busy: boolean;
  configurationReady: boolean;
  recoveryBackupId: string | null;
  onRestore: (backupId: string, transactionOnly: boolean) => Promise<void>;
}

/** The backup history's page size, matching the codebase's page-size convention. */
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

  if (error) return <p className="asb-warn-text" role="alert">{error}</p>;
  /* Stands in for the DiffView rows, so it keeps a multi-row footprint. */
  if (changes === null) {
    return <div className="asb-skeleton asb-backup-diff-loading" role="status" aria-label={t("backup.diff.loading")} />;
  }
  if (changes.length === 0) return <p className="asb-empty">{t("backup.diff.identical")}</p>;
  return <DiffView layout="list" changes={changes} label={t("backup.diff.label")} />;
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

/** One restore point on the history line: the client mark is the node, the
    reason and time lead, the hash trails, and the diff folds out beneath. */
function BackupPoint({ record, recovery, busy, configurationReady, diffOpen, onDiffOpenChange, onRestore }: {
  record: BackupRecord;
  /** The unfinished transaction's backup while configuration is not ready. */
  recovery: boolean;
  busy: boolean;
  configurationReady: boolean;
  diffOpen: boolean;
  onDiffOpenChange: (open: boolean) => void;
  onRestore: (record: BackupRecord, transactionOnly: boolean) => void;
}) {
  const { t } = useI18n();
  const diffId = useId();
  const portOnly = isGatewayPortBackup(record.reason);
  return (
    <li className="asb-backup-point" data-client={record.app} data-pending={recovery || undefined}>
      <span className="asb-backup-point-node" aria-hidden="true"><ClientLogo app={record.app} /></span>
      <div className="asb-backup-point-body">
        <p className="asb-backup-point-lead">
          <strong>{reasonLabel(record.reason)}</strong>
          <span className="asb-backup-point-time asb-code"><Time iso={record.createdAt} /></span>
        </p>
        <p className="asb-backup-point-meta">
          {clientLabel(record.app)}
          <span aria-hidden="true"> · </span>
          <span className="asb-code">{record.contentHash.slice(0, 12)}</span>
        </p>
        {portOnly && <p className="asb-backup-point-note">{t("backup.history.gatewayPortNote")}</p>}
      </div>
      <div className="asb-backup-point-actions">
        {!portOnly && (
          <Button variant="secondary" size="compact" disabled={busy || !(configurationReady || recovery)}
            onClick={() => onRestore(record, recovery)}>
            {t(recovery ? "backup.history.restoreTransaction" : "backup.history.restore")}
          </Button>
        )}
        <CollapsibleTrigger open={diffOpen} onOpenChange={(open) => { if (!busy) onDiffOpenChange(open); }}
          contentId={diffId} label={t("backup.history.viewDiff")} disabled={busy} />
      </div>
      <Collapsible open={diffOpen} id={diffId} className="asb-backup-point-diff">
        <div className="asb-backup-point-diff-body"><BackupDiff record={record} /></div>
      </Collapsible>
    </li>
  );
}

/** Backup history and the selection for an explicitly confirmed restore. */
export function BackupHistory({ records, busy, configurationReady, recoveryBackupId, onRestore }: Props) {
  const { t } = useI18n();
  const [pending, setPending] = useState<{ record: BackupRecord; transactionOnly: boolean } | null>(null);
  const [openDiffs, setOpenDiffs] = useState<ReadonlySet<string>>(() => new Set());
  const [page, setPage] = useState(1);
  const recoveryBackup = configurationReady ? undefined : records.find((record) => record.id === recoveryBackupId);
  useEffect(() => { setPage(1); }, [recoveryBackupId]);

  function setDiffOpen(recordId: string, open: boolean) {
    setOpenDiffs((current) => {
      const next = new Set(current);
      if (open) next.add(recordId);
      else next.delete(recordId);
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
  const recoveryNotice = !configurationReady && (
    <p className="asb-scope-note asb-warn-text" role="status">
      {t(recoveryBackup ? "backup.history.transactionOnly" : "backup.history.transactionUnavailable")}
    </p>
  );

  return (
    <>
      {recoveryNotice}
      {records.length > 0 && (
        <div className="asb-backup-history">
          <ol className="asb-backup-points" role="list" aria-label={t("backup.history.title")}>
            {pageRecords.map((record) => <BackupPoint key={record.id} record={record}
              recovery={record.id === recoveryBackup?.id} busy={busy} configurationReady={configurationReady}
              diffOpen={openDiffs.has(record.id)} onDiffOpenChange={(open) => setDiffOpen(record.id, open)}
              onRestore={(target, transactionOnly) => setPending({ record: target, transactionOnly })} />)}
          </ol>
          <Pagination total={records.length} page={currentPage} pageSize={BACKUP_PAGE_SIZE}
            onPageChange={turnPage} label={t("backup.history.paginationAria")} />
        </div>
      )}
      {pending && (
        <RestoreConfirmation record={pending.record} transactionOnly={pending.transactionOnly || !configurationReady}
          busy={busy} onRestore={onRestore} onClose={() => setPending(null)} />
      )}
    </>
  );
}
