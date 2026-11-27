import type {
  CodexProbeBatch, CodexProbeBatchStatus, CodexProbeRun, CodexProbeRunStatus,
} from "../api/client";
import type { MessageKey, TFunction } from "../i18n";
import { useI18n } from "../i18n";
import { formatCompactTokenCount, formatTokenCount, TOKEN_UNIT } from "../lib/token-format";
import { Table, type TableColumn } from "./Table";
import { ProbeAnswer } from "./ProbeAnswer";

export const BATCH_STATUS_LABEL: Record<CodexProbeBatchStatus, MessageKey> = {
  running: "codex.probe.statusRunning",
  completed: "codex.probe.statusCompleted",
  cancelled: "codex.probe.statusCancelled",
  failed: "codex.probe.statusFailed",
  interrupted: "codex.probe.statusInterrupted",
  "config-changed": "codex.probe.statusConfigChanged",
};

export const BATCH_STATUS_CLASS: Record<CodexProbeBatchStatus, string> = {
  running: "",
  completed: "asb-ok-text",
  cancelled: "",
  failed: "asb-fail-text",
  interrupted: "asb-warn-text",
  "config-changed": "asb-warn-text",
};

export const RUN_STATUS_LABEL: Record<CodexProbeRunStatus, MessageKey> = {
  running: "codex.probe.runRunning",
  passed: "codex.probe.runPassed",
  failed: "codex.probe.runFailed",
  undetermined: "codex.probe.runUndetermined",
};

export const RUN_STATUS_CLASS: Record<CodexProbeRunStatus, string> = {
  running: "",
  passed: "asb-ok-text",
  failed: "asb-fail-text",
  undetermined: "asb-warn-text",
};

export function formatDuration(totalMs: number, t: TFunction): string {
  const seconds = Math.round(totalMs / 1000);
  if (seconds < 60) return t("codex.duration.seconds", { count: seconds });
  const minutes = Math.floor(seconds / 60);
  const restSeconds = seconds % 60;
  if (minutes < 60) return restSeconds
    ? t("codex.duration.minutesSeconds", { minutes, seconds: restSeconds })
    : t("codex.duration.minutes", { minutes });
  const hours = Math.floor(minutes / 60);
  const restMinutes = minutes % 60;
  return restMinutes
    ? t("codex.duration.hoursMinutes", { hours, minutes: restMinutes })
    : t("codex.duration.hours", { hours });
}

/** The configuration the batch ran against, as one compact line. */
export function configSummary(batch: CodexProbeBatch, t: TFunction): string {
  const profile = batch.config.profileName ?? t("codex.probe.unlinkedProfile");
  const model = batch.config.profileModel ?? t("codex.probe.unknownModel");
  const effort = batch.config.reasoningEffort
    ? t("codex.probe.effortValue", { value: batch.config.reasoningEffort })
    : t("codex.probe.effortAuto");
  const connection = batch.config.connectionIdentity ?? t("codex.probe.unknownConnection");
  return `${profile} · ${model} · ${effort} · ${connection}`;
}

export interface ProbeRunSummary {
  passedCount: number;
  judgedCount: number;
  recordedRuns: number;
  runCount: number;
  totalTokens: number | null;
  totalDurationMs: number | null;
  averageReasoning: number | null;
}

/** Aggregates that keep unknown usage unknown — a missing total is never
 * counted as zero. */
export function summarizeRuns(runs: CodexProbeRun[]): ProbeRunSummary {
  const finished = runs.filter((run) => run.status !== "running");
  const reasoningRuns = finished.filter((run) => run.reasoningTokens !== null);
  const totals = finished.flatMap((run) => run.totalTokens === null ? [] : [run.totalTokens]);
  const durations = finished.flatMap((run) => run.durationMs === null ? [] : [run.durationMs]);
  return {
    passedCount: finished.filter((run) => run.status === "passed").length,
    judgedCount: finished.filter((run) => run.status === "passed" || run.status === "failed").length,
    recordedRuns: totals.length,
    runCount: finished.length,
    totalTokens: totals.length ? totals.reduce((total, tokens) => total + tokens, 0) : null,
    totalDurationMs: durations.length
      ? durations.reduce((total, duration) => total + duration, 0)
      : null,
    averageReasoning: reasoningRuns.length
      ? Math.round(
          reasoningRuns.reduce((total, run) => total + (run.reasoningTokens ?? 0), 0) /
            reasoningRuns.length,
        )
      : null,
  };
}

function runColumns(t: TFunction, batch: CodexProbeBatch): Array<TableColumn<CodexProbeRun>> {
  return [
    {
      key: "seq",
      align: "end",
      header: t("codex.probe.colSeq"),
      render: (run) => `${run.seq}`,
    },
    {
      key: "result",
      header: t("codex.probe.colGrading"),
      cellClassName: "asb-codex-probe-detail",
      render: (run) => run.executionError ? (
        <span className={RUN_STATUS_CLASS[run.status]}>
          {t(RUN_STATUS_LABEL[run.status])} · {run.executionError}
        </span>
      ) : (
        <span className={batch.gradingVersion !== "1" && run.status === "undetermined"
          ? "" : RUN_STATUS_CLASS[run.status]}>
          {t(batch.gradingVersion !== "1" && run.status === "undetermined"
            ? "codex.probe.visual.manualReview" : RUN_STATUS_LABEL[run.status])}
        </span>
      ),
    },
    {
      key: "answer",
      header: t("codex.probe.colAnswer"),
      cellClassName: batch.gradingVersion !== "1" ? "asb-probe-answer-cell" : undefined,
      render: (run) => <ProbeAnswer answer={run.finalAnswer} gradingVersion={batch.gradingVersion} seq={run.seq} />,
    },
    {
      key: "reasoning",
      align: "end",
      header: t("codex.probe.colReasoning", { unit: TOKEN_UNIT }),
      render: (run) =>
        run.reasoningTokens === null ? "—" : formatTokenCount(run.reasoningTokens),
    },
    {
      key: "total",
      align: "end",
      header: t("codex.probe.colTotal", { unit: TOKEN_UNIT }),
      cellClassName: "asb-codex-probe-detail",
      render: (run) => <>
        {run.totalTokens === null ? "—" : formatTokenCount(run.totalTokens)}
        {run.usageError && <p className="asb-codex-probe-meta">{t("codex.probe.usageUnavailable", { error: run.usageError })}</p>}
      </>,
    },
    {
      key: "model",
      header: t("codex.probe.colModel"),
      cellClassName: "asb-code",
      render: (run) => run.reportedModel ?? "—",
    },
    {
      key: "duration",
      align: "end",
      header: t("codex.probe.colDuration"),
      render: (run) => run.durationMs === null ? "—" : formatDuration(run.durationMs, t),
    },
  ];
}

/** The per-run detail table shared by the live panel and history detail. */
export function ProbeRunsTable({ batch }: { batch: CodexProbeBatch }) {
  const { t } = useI18n();
  const columns = runColumns(t, batch);
  return (
    <Table columns={columns} rows={batch.runs} rowKey={(run) => `probe-run-${batch.batchId}-${run.seq}`}
      ariaLabel={t("codex.probe.runsTableAria")} className="asb-codex-probe-table" />
  );
}

/** The summary card band shared by the live panel and history detail. */
export function probeSummaryCards(batch: CodexProbeBatch, t: TFunction) {
  const summary = summarizeRuns(batch.runs);
  return [
    batch.gradingVersion !== "1"
      ? { label: t("codex.probe.visual.cardReview"), value: t("codex.probe.visual.manualReview") }
      : { label: t("codex.probe.cardPassed"), value: `${summary.passedCount}/${summary.judgedCount}` },
    {
      label: summary.recordedRuns === summary.runCount && summary.runCount > 0
        ? t("codex.probe.cardMeasuredUsage") : t("codex.probe.cardRecordedUsage"),
      value: summary.totalTokens === null ? "—" : formatCompactTokenCount(summary.totalTokens),
      unit: TOKEN_UNIT,
      detail: summary.recordedRuns < summary.runCount
        ? t("codex.probe.usageCoverage", { recorded: summary.recordedRuns, total: summary.runCount }) : undefined,
    },
    {
      label: t("codex.probe.cardAvgReasoning"),
      value: summary.averageReasoning === null ? "—" : formatCompactTokenCount(summary.averageReasoning),
      unit: TOKEN_UNIT,
    },
    { label: t("codex.probe.cardTotalTime"), value: summary.totalDurationMs === null ? "—" : formatDuration(summary.totalDurationMs, t) },
  ];
}
