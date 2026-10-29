import { commandErrorText } from "../i18n/errors";
import {
  type RuntimeLogAction,
  type RuntimeLogEntry,
} from "../api/client";
import { Pagination } from "../components/Pagination";
import { Table, type TableColumn } from "../components/Table";
import { Time } from "../components/Time";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { ScrollTextIcon } from "../components/icons";
import { Button } from "../components/Button";
import { useI18n, type MessageKey, type TFunction } from "../i18n";
import { RUNTIME_LOG_PAGE_SIZE, levelLabel, useRuntimeLogs } from "./use-runtime-logs";

const ACTION_LABEL: Record<RuntimeLogAction, MessageKey> = {
  appStarted: "backup.action.appStarted",
  storageCacheMaintained: "backup.action.storageCacheMaintained",
  storageHistoryArchived: "backup.action.storageHistoryArchived",
  storageBackupsCleaned: "backup.action.storageBackupsCleaned",
  storageSessionsCleaned: "backup.action.storageSessionsCleaned",
  updateTempSwept: "backup.action.updateTempSwept",
  appSettingsSaved: "backup.action.appSettingsSaved",
  outboundProxySaved: "backup.action.outboundProxySaved",
  outboundProxyRepaired: "backup.action.outboundProxyRepaired",
  codexSubagentSettingsApplied: "backup.action.codexSubagentSettingsApplied",
  providersSqlImported: "backup.action.providersSqlImported",
  appSettingsRepaired: "backup.action.appSettingsRepaired",
  applicationDataRepaired: "backup.action.applicationDataRepaired",
  profileStoreRepaired: "backup.action.profileStoreRepaired",
  profileCreated: "backup.action.profileCreated",
  profileUpdated: "backup.action.profileUpdated",
  profileDeleted: "backup.action.profileDeleted",
  profilesReordered: "backup.action.profilesReordered",
  profileImported: "backup.action.profileImported",
  globalPromptDocumentSaved: "backup.action.globalPromptDocumentSaved",
  configurationSwitched: "backup.action.configurationSwitched",
  backupRestored: "backup.action.backupRestored",
  switchUndone: "backup.action.switchUndone",
  staleLockRecovered: "backup.action.staleLockRecovered",
  cloudBackupSettingsSaved: "backup.action.cloudBackupSettingsSaved",
  cloudBackupConnectionTested: "backup.action.cloudBackupConnectionTested",
  cloudBackupUploaded: "backup.action.cloudBackupUploaded",
  cloudBackupRestored: "backup.action.cloudBackupRestored",
  sessionResumed: "backup.action.sessionResumed",
  sessionDeleted: "backup.action.sessionDeleted",
  ccSwitchProfilesImported: "backup.action.ccSwitchProfilesImported",
  officialLoginCompleted: "backup.action.officialLoginCompleted",
};

function logColumns(t: TFunction): Array<TableColumn<RuntimeLogEntry>> {
  return [
    {
      key: "at",
      header: t("backup.col.time"),
      cellClassName: "asb-code",
      render: (entry) => <Time iso={entry.at} />,
    },
    {
      key: "level",
      header: t("backup.col.level"),
      render: (entry) => (
        <span className={`asb-runtime-log-level is-${entry.level}`}>{levelLabel(entry.level)}</span>
      ),
    },
    {
      key: "action",
      header: t("backup.col.event"),
      cellClassName: "asb-runtime-log-action",
      render: (entry) => t(ACTION_LABEL[entry.action]),
    },
    {
      key: "errorCode",
      header: t("backup.col.errorCode"),
      cellClassName: "asb-code",
      render: (entry) => entry.errorCode ?? "—",
    },
  ];
}

/** Read-only view of the application's own bounded diagnostic event files.
 * The record-level setting, level filter and refresh actions live in the
 * diagnostics page header; the card renders the current page of entries. */
export function LogsPage({ logs }: { logs: ReturnType<typeof useRuntimeLogs> }) {
  const { t } = useI18n();
  const { entries, visibleEntries, pageEntries, page, setPage, filter, loading, error, folderError, total, skipped } = logs;

  const columns = logColumns(t);

  return (
    <section className="asb-panel asb-runtime-logs" aria-labelledby="runtime-logs-heading">
      <ModuleHeader id="runtime-logs-heading" title={t("backup.logs.title")} />
      {total > entries.length && <p className="asb-scope-note">{t("backup.logs.coverage", { shown: entries.length, total })}</p>}
      {skipped > 0 && <p className="asb-warn-text" role="alert">{t("backup.logs.skipped", { count: skipped })}</p>}
      {error && (
        <p className="asb-runtime-log-notice" role="alert">
          {t("backup.logs.readError", { message: commandErrorText(error, t) })}
        </p>
      )}
      {folderError && (
        <p className="asb-runtime-log-notice" role="alert">
          {t("backup.logs.folderError", { message: commandErrorText(folderError, t) })}
        </p>
      )}
      {loading && entries.length === 0 ? (
        <div className="asb-runtime-log-skeleton" role="status" aria-label={t("backup.logs.loadingAria")}>
          {Array.from({ length: RUNTIME_LOG_PAGE_SIZE }, (_, index) => (
            <span key={index} className="asb-skeleton asb-runtime-log-skeleton-row" />
          ))}
        </div>
      ) : visibleEntries.length === 0 ? (
        <div className="asb-empty-state asb-runtime-log-empty">
          <span className="asb-empty-state-icon" aria-hidden="true">
            <ScrollTextIcon />
          </span>
          <h3 className="asb-section-title">
            {filter === "all" ? t("backup.logs.empty") : t("backup.logs.emptyLevel", { level: levelLabel(filter) })}
          </h3>
          {filter !== "all" && <Button variant="secondary" onClick={() => logs.setFilter("all")}>{t("common.clearFilters")}</Button>}
        </div>
      ) : (
        <>
          <Table
            columns={columns}
            rows={pageEntries}
            rowKey={(entry, index) => `${entry.at}-${entry.action}-${entry.errorCode ?? ""}-${index}`}
            ariaLabel={t("backup.logs.tableAria")}
            className="asb-runtime-log-table"
          />
          <Pagination
            total={visibleEntries.length}
            page={page}
            pageSize={RUNTIME_LOG_PAGE_SIZE}
            onPageChange={setPage}
            label={t("backup.logs.paginationAria")}
          />
        </>
      )}
    </section>
  );
}
