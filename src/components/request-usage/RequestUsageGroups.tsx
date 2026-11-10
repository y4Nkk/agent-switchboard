import type { RequestUsageGroup, RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Table, type TableColumn } from "../Table";
import { Pagination } from "../Pagination";
import { cost, duration, number, percent } from "./format";

export function RequestUsageGroups({ report, onPage }: { report: RequestUsageReport; onPage: (page: number) => void }) {
  const { t, language } = useI18n();
  const timingColumns: TableColumn<RequestUsageGroup>[] = report.scope === "generation" ? [{ key: "speed", header: t("usagePerformance.speedColumn"), align: "end", render: (row) => <>
      {number(row.metrics.outputRateP50, language)}
      <span className="asb-request-usage-caption">P10 {number(row.metrics.outputRateP10, language)} · n={row.metrics.speedSamples}</span></> },
    { key: "first", header: t("usagePerformance.firstColumn"), align: "end", render: (row) => <>
      {duration(row.metrics.firstTokenP50Ms, language)}
      <span className="asb-request-usage-caption">P95 {duration(row.metrics.firstTokenP95Ms, language)} · n={row.metrics.firstTokenSamples}</span></> }]
      : [{ key: "duration", header: t("usagePerformance.duration"), align: "end", render: (row) => <>
        {duration(row.metrics.durationP50Ms, language)} / {duration(row.metrics.durationP95Ms, language)}
        <span className="asb-request-usage-caption">n={row.metrics.durationSamples}</span></> }];
  const columns: TableColumn<RequestUsageGroup>[] = [
    { key: "client", header: t("usage.column.client"), render: (row) => clientName(row.app) },
    { key: "model", header: t("usage.column.model"), render: (row) => <>
      <span className="asb-code">{row.model ?? t("usage.model.unrecorded")}</span>
      <span className="asb-request-usage-caption">{row.eventName}</span></> },
    { key: "records", header: t("usagePerformance.recordsCount"), align: "end", render: (row) => number(row.metrics.records, language) },
    ...timingColumns,
    { key: "success", header: t("usagePerformance.successRatio"), align: "end", render: (row) => percent(row.metrics.successRatio, language) },
    { key: "cache", header: t("usagePerformance.cache"), align: "end", render: (row) => <>
      {percent(row.metrics.cacheHitRate, language)}<span className="asb-request-usage-caption">n={row.metrics.cacheSamples}</span></> },
    { key: "cost", header: t("usagePerformance.cost"), align: "end", render: (row) => <>
      {cost(row.metrics.estimatedCostUsd, language)}<span className="asb-request-usage-caption">n={row.metrics.costSamples}</span></> },
  ];
  return <div className="asb-request-usage-section">
    <Table columns={columns} rows={report.groups} ariaLabel={t("usagePerformance.groups")}
      rowKey={(row) => JSON.stringify([row.app, row.model, row.eventName])} className="asb-request-usage-group-table" />
    <Pagination total={report.totalGroups} page={report.groupPage} pageSize={report.groupPageSize}
      onPageChange={onPage} label={t("usagePerformance.groupPages")} />
  </div>;
}
