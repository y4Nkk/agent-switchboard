import { useEffect, useId, useState } from "react";
import type { RequestUsageReport, RequestUsageRow } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { FactList } from "../FactList";
import { Pagination } from "../Pagination";
import { Table, type TableColumn } from "../Table";
import { Time } from "../Time";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageRows({ report, onPage }: { report: RequestUsageReport; onPage: (page: number) => void }) {
  const { t, language } = useI18n();
  const id = useId();
  const [expanded, setExpanded] = useState<string | null>(null);
  useEffect(() => { setExpanded(null); }, [report.page, report.scope]);
  const columns: TableColumn<RequestUsageRow>[] = [
    { key: "at", header: t("usagePerformance.time"), render: (row) => <Time iso={new Date(row.atMs).toISOString()} precision="second" /> },
    { key: "model", header: t("usagePerformance.clientModel"), render: (row) => <>
      {clientName(row.app)}<span className="asb-request-usage-caption asb-code">{row.model ?? t("usage.model.unrecorded")}</span></> },
    { key: "outcome", header: t("usagePerformance.outcome"), render: (row) => <span className={`asb-request-outcome is-${row.outcome}`}>
      {t(row.eventKind === "response.incomplete" ? "usagePerformance.incomplete" : `usagePerformance.outcome.${row.outcome}`)}
      <span className="asb-request-usage-caption">{row.status ?? t("usagePerformance.noStatus")}</span></span> },
    ...(report.scope === "generation" ? [{ key: "first", header: t("usagePerformance.firstColumn"),
      align: "end", render: (row: RequestUsageRow) => duration(row.firstTokenMs, language) }] as TableColumn<RequestUsageRow>[] : []),
    { key: "duration", header: t("usagePerformance.durationColumn"), align: "end", render: (row) => duration(row.durationMs, language) },
    ...(report.scope === "generation" ? [{ key: "speed", header: t("usagePerformance.speedColumn"),
      align: "end", render: (row: RequestUsageRow) => number(row.outputTokensPerSecond, language) }] as TableColumn<RequestUsageRow>[] : []),
    { key: "tokens", header: t("usagePerformance.inputOutput"), align: "end", render: (row) => `${tokens(row.inputTokens)} / ${tokens(row.outputTokens)}` },
    { key: "cost", header: t("usagePerformance.cost"), align: "end", render: (row) => cost(row.estimatedCostUsd, language) },
    { key: "retry", header: t("usagePerformance.retryCount"), align: "end", render: (row) => number(
      row.attemptCount === null ? null : row.attemptCount - 1, language) },
    { key: "details", header: t("usagePerformance.details"), render: (row) => <CollapsibleTrigger
      open={expanded === row.id} contentId={`${id}-${row.id}`} label={t("usagePerformance.details")}
      onOpenChange={(open) => setExpanded(open ? row.id : null)} /> },
  ];
  return <div className="asb-request-usage-section">
    <Table columns={columns} rows={report.rows} rowKey={(row) => row.id}
      ariaLabel={t("usagePerformance.records")} className="asb-request-usage-row-table"
      renderExpansion={(row) => <Collapsible open={expanded === row.id} id={`${id}-${row.id}`}>
        <RequestDetails row={row} />
      </Collapsible>} />
    <Pagination total={report.summary.records} page={report.page} pageSize={report.pageSize}
      onPageChange={onPage} label={t("usagePerformance.recordPages")} />
  </div>;
}

function RequestDetails({ row }: { row: RequestUsageRow }) {
  const { t, language } = useI18n();
  return <div className="asb-request-usage-details"><FactList rows={[
    { label: t("usagePerformance.eventName"), value: row.eventName },
    { label: t("usagePerformance.eventKind"), value: row.eventKind ?? "—" },
    { label: t("usagePerformance.timeSource"), value: t(`usagePerformance.timeSource.${row.timeSource}`) },
    { label: t("usagePerformance.measurementScope"), value: t(`usagePerformance.scope.${row.scope}`) },
    { label: t("usagePerformance.sessionId"), value: row.sessionId ?? "—" },
    { label: t("usagePerformance.requestId"), value: row.requestId ?? "—" },
    { label: t("usagePerformance.errorType"), value: row.errorType ?? "—" },
    { label: t("usagePerformance.firstContent"), value: duration(row.firstContentMs, language) },
    { label: t("usagePerformance.token.cacheRead"), value: tokens(row.cacheReadTokens) },
    { label: t("usagePerformance.token.cacheCreation"), value: tokens(row.cacheCreationTokens) },
    { label: t("usagePerformance.token.reasoning"), value: tokens(row.reasoningTokens) },
    { label: t("usagePerformance.attempts"), value: number(row.attemptCount, language) },
  ]} /></div>;
}
