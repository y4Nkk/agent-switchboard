import type { AppKind, RequestUsageGroup, RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Table, type TableColumn } from "../Table";
import { Pagination } from "../Pagination";
import { Button } from "../Button";
import { cost, duration, number, percent } from "./format";

export type ProviderName = (app: AppKind, id: string) => string;

export function RequestUsageGroups({ report, providerName, onPage, onProvider }: {
  report: RequestUsageReport; providerName: ProviderName; onPage: (page: number) => void;
  onProvider: (app: AppKind, id: string) => void;
}) {
  const { t, language } = useI18n();
  const columns: TableColumn<RequestUsageGroup>[] = [
    { key: "provider", header: t("usagePerformance.provider"), render: (row) => <>
      <Button variant="ghost" size="compact" onClick={() => onProvider(row.app, row.profileId)}
        aria-label={t("usagePerformance.filterProvider", { name: providerName(row.app, row.profileId) })}>
        {providerName(row.app, row.profileId)}</Button>
      <span className="asb-request-usage-caption">{clientName(row.app)}</span></> },
    { key: "model", header: t("usage.column.model"), render: (row) => <>
      <span className="asb-code">{row.model ?? t("usage.model.unrecorded")}</span>
      <span className="asb-request-usage-caption">{t(`usagePerformance.modelSource.${row.modelSource}`)}</span></> },
    { key: "requests", header: t("usagePerformance.requests"), align: "end", render: (row) => number(row.metrics.requests, language) },
    { key: "speed", header: t("usagePerformance.speedColumn"), align: "end", render: (row) => <>
      {number(row.metrics.outputRateP50, language)}
      <span className="asb-request-usage-caption">P10 {number(row.metrics.outputRateP10, language)} · n={row.metrics.speedSamples}</span></> },
    { key: "first", header: t("usagePerformance.firstColumn"), align: "end", render: (row) => <>
      {duration(row.metrics.firstOutputP50Ms, language)}
      <span className="asb-request-usage-caption">P95 {duration(row.metrics.firstOutputP95Ms, language)} · n={row.metrics.firstOutputSamples}</span></> },
    { key: "completion", header: t("usagePerformance.completion"), align: "end", render: (row) => percent(row.metrics.completionRate, language) },
    { key: "cache", header: t("usagePerformance.cache"), align: "end", render: (row) => <>
      {percent(row.metrics.cacheHitRate, language)}<span className="asb-request-usage-caption">n={row.metrics.cacheSamples}</span></> },
    { key: "cost", header: t("usagePerformance.cost"), align: "end", render: (row) => <>
      {cost(row.metrics.estimatedCostUsd, language)}<span className="asb-request-usage-caption">n={row.metrics.costSamples}</span></> },
  ];
  return <section className="asb-request-usage-section">
    <h3 className="asb-section-title">{t("usagePerformance.groups")}</h3>
    <Table columns={columns} rows={report.groups} ariaLabel={t("usagePerformance.groups")}
      rowKey={(row) => JSON.stringify([row.app, row.profileId, row.model, row.modelSource])} className="asb-request-usage-group-table" />
    <Pagination total={report.totalGroups} page={report.groupPage} pageSize={report.groupPageSize}
      onPageChange={onPage} label={t("usagePerformance.groupPages")} />
  </section>;
}
