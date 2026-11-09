import { useEffect, useId, useState } from "react";
import type { RequestUsageReport, RequestUsageRow } from "../../api/client";
import { useI18n } from "../../i18n";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { FactList } from "../FactList";
import { Pagination } from "../Pagination";
import { Table, type TableColumn } from "../Table";
import { Time } from "../Time";
import type { ProviderName } from "./RequestUsageGroups";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageRows({ report, providerName, onPage }: {
  report: RequestUsageReport; providerName: ProviderName; onPage: (page: number) => void;
}) {
  const { t, language } = useI18n();
  const id = useId();
  const [expanded, setExpanded] = useState<string | null>(null);
  useEffect(() => { setExpanded(null); }, [report.page]);
  const columns: TableColumn<RequestUsageRow>[] = [
    { key: "at", header: t("usagePerformance.time"), render: (row) => <Time iso={new Date(row.atMs).toISOString()} precision="second" /> },
    { key: "provider", header: t("usagePerformance.providerModel"), render: (row) => <>
      {providerName(row.app, row.profileId)}<span className="asb-request-usage-caption asb-code">{row.model ?? t("usage.model.unrecorded")}</span></> },
    { key: "outcome", header: t("usagePerformance.outcome"), render: (row) => <span className={`asb-request-outcome is-${row.outcome}`}>
      {t(row.status === 206 ? "usagePerformance.partialOutput" : `usagePerformance.outcome.${row.outcome}`)}
      <span className="asb-request-usage-caption">{row.status ?? t("usagePerformance.noStatus")}</span></span> },
    { key: "first", header: t("usagePerformance.firstColumn"), align: "end", render: (row) => duration(row.firstOutputMs, language) },
    { key: "duration", header: t("usagePerformance.durationColumn"), align: "end", render: (row) => duration(row.durationMs, language) },
    { key: "speed", header: t("usagePerformance.speedColumn"), align: "end", render: (row) => number(row.outputTokensPerSecond, language) },
    { key: "tokens", header: t("usagePerformance.inputOutput"), align: "end", render: (row) => `${tokens(row.inputTokens)} / ${tokens(row.outputTokens)}` },
    { key: "cost", header: t("usagePerformance.cost"), align: "end", render: (row) => cost(row.estimatedCostUsd, language) },
    { key: "retry", header: t("usagePerformance.retryCount"), align: "end", render: (row) => number(
      row.attempts.length === 0 ? null : row.attempts.length - 1, language) },
    { key: "details", header: t("usagePerformance.details"), render: (row) => <CollapsibleTrigger
      open={expanded === row.id} contentId={`${id}-${row.id}`} label={t("usagePerformance.details")}
      onOpenChange={(open) => setExpanded(open ? row.id : null)} /> },
  ];
  return <div className="asb-request-usage-section">
    <Table columns={columns} rows={report.rows} rowKey={(row) => `${row.app}:${row.id}`}
      ariaLabel={t("usagePerformance.records")} className="asb-request-usage-row-table"
      renderExpansion={(row) => <Collapsible open={expanded === row.id} id={`${id}-${row.id}`}>
        <RequestDetails row={row} providerName={providerName} />
      </Collapsible>} />
    <Pagination total={report.summary.requests} page={report.page} pageSize={report.pageSize}
      onPageChange={onPage} label={t("usagePerformance.recordPages")} />
  </div>;
}

function RequestDetails({ row, providerName }: { row: RequestUsageRow; providerName: ProviderName }) {
  const { t } = useI18n();
  return <div className="asb-request-usage-details"><FactList rows={[
    { label: t("usagePerformance.modelSource"), value: t(`usagePerformance.modelSource.${row.modelSource}`) },
    { label: t("usagePerformance.requestModel"), value: row.requestModel ?? "—" },
    { label: t("usagePerformance.mappedModel"), value: row.mappedModel ?? "—" },
    { label: t("usagePerformance.responseModel"), value: row.responseModel ?? "—" },
    { label: t("usagePerformance.protocol"), value: row.upstreamProtocol ? t(`usagePerformance.protocol.${row.upstreamProtocol}`) : "—" },
    { label: t("usagePerformance.token.cacheRead"), value: tokens(row.cacheReadTokens) },
    { label: t("usagePerformance.token.cacheCreation"), value: tokens(row.cacheCreationTokens) },
    { label: t("usagePerformance.token.reasoning"), value: tokens(row.reasoningTokens) },
    { label: t("usagePerformance.attempts"), value: row.attempts.length === 0 ? t("usagePerformance.notRecorded")
      : row.attempts.map((attempt, index) => <span className="asb-request-usage-caption" key={index}>
        {index + 1}. {providerName(row.app, attempt.profileId)} · {attempt.status ?? t("usagePerformance.noStatus")}</span>) },
    { label: t("usagePerformance.pricingError"), value: row.pricingError ?? "—" },
  ]} /></div>;
}
