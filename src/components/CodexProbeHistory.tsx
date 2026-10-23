import { useMemo, useState } from "react";
import type { CodexProbeBatch, CodexProbeGradingVersion, CodexProbeHistoryItem, CodexProbeQuestionSnapshot } from "../api/client";
import type { MessageKey, TFunction } from "../i18n";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import { ConfirmSheet } from "./ConfirmSheet";
import { EditorFrame } from "./EditorFrame";
import { Pagination } from "./Pagination";
import { ProbeTaskProgress } from "./ProbeTaskProgress";
import { Select } from "./Select";
import { StatCards } from "./charts/stat-cards";
import {
  BATCH_STATUS_CLASS, BATCH_STATUS_LABEL, ProbeRunsTable, configSummary, probeSummaryCards,
  summarizeRuns,
} from "./probe-shared";
import { Table, type TableColumn } from "./Table";
import { Time } from "./Time";
import { ModuleHeader } from "./WorkspaceHeader";
import { formatCompactTokenCount, TOKEN_UNIT } from "../lib/token-format";
import type { useCodexProbe } from "../app/useCodexProbe";
import { useCodexProbeHistory } from "./use-codex-probe-history";

interface Props {
  active: boolean;
  probe: ReturnType<typeof useCodexProbe>;
  onClose: () => void;
  /** Fills the radar form with this question; the user starts it against
   * the current configuration. */
  onReuseQuestion: (question: CodexProbeQuestionSnapshot, runCount: number, grading: CodexProbeGradingVersion) => void;
  /** Notifies the radar panel that history changed (e.g. deletions). */
  onChanged: () => void;
}

const RANGE_OPTIONS: Array<{ value: string; labelKey: MessageKey }> = [
  { value: "all", labelKey: "codex.history.rangeAll" },
  { value: "last7Days", labelKey: "codex.history.range7" },
  { value: "last30Days", labelKey: "codex.history.range30" },
];

const STATUS_OPTIONS: Array<{ value: string; labelKey: MessageKey }> = [
  { value: "all", labelKey: "codex.history.statusAll" },
  { value: "running", labelKey: "codex.probe.statusRunning" },
  { value: "completed", labelKey: "codex.probe.statusCompleted" },
  { value: "cancelled", labelKey: "codex.probe.statusCancelled" },
  { value: "failed", labelKey: "codex.probe.statusFailed" },
  { value: "interrupted", labelKey: "codex.probe.statusInterrupted" },
  { value: "config-changed", labelKey: "codex.probe.statusConfigChanged" },
];

function tokensCell(row: CodexProbeHistoryItem, t: TFunction) {
  if (row.totalTokens === null) return row.runCount === 0 ? "—" : t("codex.history.unknown");
  const coverage = row.recordedRuns < row.runCount
    ? t("codex.history.coverage", { recorded: row.recordedRuns, total: row.runCount })
    : "";
  return `${formatCompactTokenCount(row.totalTokens)} ${TOKEN_UNIT}${coverage}`;
}

function historyColumns(history: ReturnType<typeof useCodexProbeHistory>, t: TFunction): Array<TableColumn<CodexProbeHistoryItem>> {
  return [
    {
      key: "select",
      header: t("codex.history.colSelect"),
      render: (row) => <Checkbox checked={history.selection.has(row.batchId)}
        disabled={row.status === "running" || history.deleting} label=""
        ariaLabel={t("codex.history.selectAria", { time: row.startedAt })}
        onChange={(checked) => history.toggleSelected(row.batchId, checked)} />,
    },
    {
      key: "time",
      header: t("codex.history.colTime"),
      render: (row) => <Time iso={row.startedAt} />,
    },
    {
      key: "config",
      header: t("codex.history.colConfig"),
      cellClassName: "asb-codex-probe-detail",
      render: (row) => row.profileName ?? t("codex.probe.unlinkedProfile"),
    },
    {
      key: "question",
      header: t("codex.history.colQuestion"),
      render: (row) => row.questionLabel,
    },
    {
      key: "status",
      header: t("codex.history.colStatus"),
      render: (row) => <span className={BATCH_STATUS_CLASS[row.status]}>
        {t(BATCH_STATUS_LABEL[row.status])}
      </span>,
    },
    {
      key: "passed",
      header: t("codex.history.colPassed"),
      render: (row) => row.gradingVersion !== "1" ? t("codex.probe.visual.manualReview")
        : `${row.passedCount}/${row.judgedCount}`,
    },
    {
      key: "tokens",
      header: t("codex.history.colTokens"),
      cellClassName: "asb-num",
      render: (row) => tokensCell(row, t),
    },
    {
      key: "view",
      header: t("codex.history.colActions"),
      render: (row) => <Button variant="secondary" onClick={() => history.openDetail(row.batchId)}>
        {t("codex.history.view")}
      </Button>,
    },
  ];
}

function HistoryList({ history, onRequestDelete }: {
  history: ReturnType<typeof useCodexProbeHistory>;
  onRequestDelete: (batchIds: string[]) => void;
}) {
  const { t } = useI18n();
  const columns = historyColumns(history, t);
  return (
    <div className="asb-codex-probe-history">
      <div className="asb-model-usage-controls">
        <Select value={history.range} options={RANGE_OPTIONS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
          ariaLabel={t("codex.history.rangeAria")}
          onChange={(value) => history.changeRange(value as typeof history.range)} />
        <Select value={history.profileFilter} options={[
          { value: "all", label: t("codex.history.profileAll") },
          { value: "unlinked", label: t("codex.probe.unlinkedProfile") },
          ...(history.profiles ?? []).map((profile) => ({
            value: profile.profileId,
            label: profile.profileName,
          })),
        ]} placeholder={history.profiles === null ? t("codex.history.profilesLoading") : t("codex.history.chooseProfile")}
          ariaLabel={t("codex.history.profileAria")} onChange={history.changeProfile} />
        <Select value={history.statusFilter} options={STATUS_OPTIONS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
          ariaLabel={t("codex.history.statusAria")}
          onChange={history.changeStatus} />
        <Button variant="secondary" disabled={history.loading} onClick={history.refresh}>{t("codex.history.refresh")}</Button>
        <Button variant="danger" disabled={history.selection.size === 0 || history.deleting}
          onClick={() => onRequestDelete([...history.selection])}>
          {t("codex.history.deleteSelected", { count: history.selection.size })}
        </Button>
      </div>
      {history.profilesError && <p className="asb-warn-text" role="alert">
        {t("codex.history.profilesError", { error: history.profilesError })}
      </p>}
      {history.listError && <p className="asb-warn-text" role="alert">{t("codex.history.listError", { error: history.listError })}</p>}
      {history.deleteError && <p className="asb-warn-text" role="alert">{t("codex.history.deleteError", { error: history.deleteError })}</p>}
      <p className="asb-codex-probe-meta asb-num" role="status">
        {t("codex.history.totalCount", { count: history.total })}{history.loading ? t("codex.history.loadingSuffix") : ""}
      </p>
      {history.items.length > 0 && <>
        <Table columns={columns} rows={history.items} ariaLabel={t("codex.history.listAria")}
          rowKey={(row) => row.batchId} />
        <Pagination total={history.total} page={history.page} pageSize={history.pageSize}
          onPageChange={history.setPage} label={t("codex.history.paginationAria")} />
      </>}
      {history.items.length === 0 && !history.loading && (
        <div className="asb-empty-state">
          <p className="asb-section-title">
            {history.total === 0 && history.range === "all" && history.profileFilter === "all"
              && history.statusFilter === "all"
              ? t("codex.history.emptyAll")
              : t("codex.history.emptyFiltered")}
          </p>
          {(history.range !== "all" || history.profileFilter !== "all" || history.statusFilter !== "all") &&
            <Button variant="secondary" onClick={history.clearFilters}>{t("codex.history.clearFilters")}</Button>}
        </div>
      )}
    </div>
  );
}

function HistoryDetail({ history, onReuseQuestion, liveBatch }: {
  history: ReturnType<typeof useCodexProbeHistory>;
  onReuseQuestion: Props["onReuseQuestion"];
  liveBatch: CodexProbeBatch | null;
}) {
  const { t } = useI18n();
  const batch = liveBatch?.batchId === history.detailId ? liveBatch : history.detail;
  const summary = batch ? summarizeRuns(batch.runs) : null;
  return (
    <div className="asb-codex-probe-history">
      <ModuleHeader title={t("codex.history.detailTitle")} primaryActions={
        <>
          {batch?.status === "running" &&
            <Button variant="secondary" disabled={history.detailLoading}
              onClick={() => history.detailId && history.openDetail(history.detailId)}>
              {t("codex.history.refresh")}
            </Button>}
        </>
      } />
      {history.detailLoading && <p className="asb-codex-probe-meta" role="status">{t("codex.history.detailLoading")}</p>}
      {history.detailError && <p className="asb-warn-text" role="alert">{history.detailError}</p>}
      {batch && <>
        <p className="asb-codex-probe-meta">
          {t("codex.history.statusPrefix")}<span className={BATCH_STATUS_CLASS[batch.status]}>{t(BATCH_STATUS_LABEL[batch.status])}</span>
          {" · "}{t("codex.probe.startedAt")} <Time iso={batch.startedAt} />
          {batch.finishedAt && <> · {t("codex.probe.finishedAt")} <Time iso={batch.finishedAt} /></>}
        </p>
        {batch.statusError && <p className="asb-warn-text" role="alert">{batch.statusError}</p>}
        {batch.persistPending && <p className="asb-warn-text" role="alert">
          {batch.persistError ?? t("codex.history.persistPending")}
        </p>}
        <p className="asb-codex-probe-meta">
          {t("codex.probe.configPrefix")}{configSummary(batch, t)}{batch.cliVersion && ` · CLI ${batch.cliVersion}`}
        </p>
        <div className="asb-codex-probe-question">
          <p className="asb-codex-probe-meta">
            {t(batch.gradingVersion !== "1" ? "codex.probe.visual.questionMeta" : "codex.history.questionMeta", {
              label: batch.question.label,
              version: batch.gradingVersion,
              answer: batch.question.expectedAnswer,
            })}
          </p>
          <p className="asb-probe-question-text">{batch.question.text}</p>
        </div>
        {summary && summary.runCount > 0 && <>
          <div role="group" aria-label={t("codex.probe.summaryAria")}><StatCards stats={probeSummaryCards(batch, t)} /></div>
          {summary.recordedRuns < summary.runCount && <p className="asb-codex-probe-note" role="status">
            {t("codex.probe.partialUsage", { recorded: summary.recordedRuns, total: summary.runCount })}
          </p>}
          <ProbeRunsTable batch={batch} />
        </>}
        {summary && summary.runCount === 0 && !history.detailLoading &&
          <p className="asb-codex-probe-meta">{t("codex.history.noRuns")}</p>}
        <div className="asb-model-usage-controls">
          <Button variant="primary" onClick={() => onReuseQuestion(batch.question, batch.plannedRuns, batch.gradingVersion)}>
            {t("codex.history.reuseQuestion")}
          </Button>
        </div>
        <p className="asb-codex-probe-note">
          {t("codex.history.reuseNote")}
        </p>
      </>}
    </div>
  );
}

/** The full-page probe history on the shared editor frame: filtered and
 * paged list, detail view, explicit deletions, and a live entry back to a
 * running batch. */
export function CodexProbeHistory({ active, probe, onClose, onReuseQuestion, onChanged }: Props) {
  const { t } = useI18n();
  const batch = probe.status;
  const revision = `${batch?.batchId}:${batch?.status}:${batch?.completedRuns}:${batch?.persistPending}`;
  const history = useCodexProbeHistory(active, revision);
  const [pendingDelete, setPendingDelete] = useState<string[] | null>(null);
  const pendingItems = useMemo(
    () => history.items.filter((item) => pendingDelete?.includes(item.batchId)),
    [history.items, pendingDelete],
  );
  const running = probe.running && probe.status;
  const confirmDelete = async () => {
    const batchIds = pendingDelete ?? [];
    const deleted = await history.deleteSelected(batchIds);
    if (deleted) {
      setPendingDelete(null);
      onChanged();
      if (history.detailId && batchIds.includes(history.detailId)) {
        history.closeDetail();
      }
    }
  };
  return (
    <EditorFrame
      title={t("codex.history.title")}
      backLabel={t(history.detailId === null ? "codex.history.backToRadar" : "codex.history.backToList")}
      onBack={history.detailId === null ? onClose : history.closeDetail}
      busy={history.deleting}
      primary={running ? (
        <span className="asb-header-status" role="status">
          {t("codex.history.runningBadge", { done: running.completedRuns, total: running.plannedRuns })}
        </span>
      ) : undefined}
    >
      {running && <ProbeTaskProgress batch={running} stale={!probe.synchronized} />}
      {probe.readError && <p className="asb-warn-text" role="alert">
        {t("codex.probe.readError", { error: probe.readError })}
      </p>}
      {history.detailId === null
        ? <HistoryList history={history} onRequestDelete={setPendingDelete} />
        : <HistoryDetail history={history} onReuseQuestion={onReuseQuestion} liveBatch={probe.status} />}
      {pendingDelete !== null && pendingItems.length > 0 && (
        <ConfirmSheet
          title={t("codex.history.deleteTitle")}
          confirmLabel={t("codex.history.confirmDelete")}
          destructive
          confirmDisabled={history.deleting}
          busy={history.deleting}
          onConfirm={() => void confirmDelete()}
          onCancel={() => setPendingDelete(null)}
        >
          <ul className="asb-dialog-details">
            {history.deleteError && <li className="asb-warn-text" role="alert">{history.deleteError}</li>}
            <li>{t("codex.history.deleteCount", { count: pendingItems.length })}</li>
            <li>{t("codex.history.timeRangeLabel")} <Time iso={pendingItems.map((item) => item.startedAt).sort()[0]} />
              {" "}{t("codex.history.timeRangeTo")} <Time iso={
                pendingItems.map((item) => item.startedAt).sort()[
                  pendingItems.length - 1
                ]
              } /></li>
            <li>{t("codex.history.deleteScope")}</li>
            <li>{t("codex.history.deleteIrreversible")}</li>
          </ul>
        </ConfirmSheet>
      )}
    </EditorFrame>
  );
}
