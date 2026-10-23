import { useMemo } from "react";
import type { ModelUsageGroup, ModelUsageReport } from "../api/client";
import { ModelUsageDistributionChart } from "../components/charts/ModelUsageDistributionChart";
import { UsageTrendChart } from "../components/charts/UsageTrendChart";
import { StatCards } from "../components/charts/stat-cards";
import { ModelUsageTable } from "../components/ModelUsageTable";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { useInformationSource, type InformationNotice } from "../components/InformationSources";
import { UsageIcon } from "../components/icons";
import { useI18n, type TFunction } from "../i18n";
import { clientName } from "../lib/client-name";
import { TOKEN_UNIT, formatCompactTokenCount, formatTokenCount } from "../lib/token-format";
import type { useModelUsageReport } from "./use-model-usage-report";

function cachedTokenCount(group: ModelUsageGroup): number {
  return group.cacheReadInputTokens + group.cacheCreationInputTokens;
}
function total(report: ModelUsageReport, selector: (group: ModelUsageGroup) => number): number {
  return report.groups.reduce((sum, group) => sum + selector(group), 0);
}
function dailyTrend(report: ModelUsageReport, t: TFunction) {
  return [
    { id: "fresh-input", label: t("usage.label.freshInput"), unit: TOKEN_UNIT,
      points: report.days.map((day) => ({ at: `${day.date}T12:00:00`, value: day.inputTokens })) },
    { id: "cache", label: t("usage.label.cache"), unit: TOKEN_UNIT,
      points: report.days.map((day) => ({ at: `${day.date}T12:00:00`,
        value: day.cacheReadInputTokens + day.cacheCreationInputTokens })) },
    { id: "output", label: t("usage.label.output"), unit: TOKEN_UNIT,
      points: report.days.map((day) => ({ at: `${day.date}T12:00:00`, value: day.outputTokens })) },
  ];
}
function composition(report: ModelUsageReport, t: TFunction) {
  return report.groups.map((group, index) => ({
    id: `${group.app}-${group.model ?? "unknown"}-${index}`,
    label: `${clientName(group.app)} · ${group.model ?? t("usage.model.unrecorded")}`, value: group.totalTokens,
  }));
}

function UsageTotals({ report }: { report: ModelUsageReport }) {
  const { t } = useI18n();
  return <div className="asb-model-usage-summary">
    <div role="group" aria-label={t("usage.summary.groupAria")}><StatCards stats={[
      { label: t("usage.label.total"), value: formatCompactTokenCount(total(report, (g) => g.totalTokens)), unit: TOKEN_UNIT },
      { label: t("usage.label.freshInput"), value: formatCompactTokenCount(total(report, (g) => g.inputTokens)), unit: TOKEN_UNIT },
      { label: t("usage.label.cache"), value: formatCompactTokenCount(total(report, cachedTokenCount)), unit: TOKEN_UNIT },
      { label: t("usage.label.output"), value: formatCompactTokenCount(total(report, (g) => g.outputTokens)), unit: TOKEN_UNIT },
    ]} /></div>
    <p className="asb-scope-note">{t("usage.coverage", {
      sessions: report.coverage.sourceSessions, files: report.coverage.scannedFiles })}</p>
    {report.coverage.sessionsWithoutUsage > 0 && <p className="asb-scope-note">
      {t("usage.coverage.missing", { count: report.coverage.sessionsWithoutUsage })}</p>}
    {report.range !== "all" && report.coverage.undatedEventTokens > 0 && <p className="asb-scope-note">
      {t("usage.undated.excluded", { count: formatTokenCount(report.coverage.undatedEventTokens) })}</p>}
    {report.unassignedTokens.totalTokens > 0 && <p className="asb-model-usage-undated" role="status">
      {t("usage.undated", { count: formatTokenCount(report.unassignedTokens.totalTokens) })}</p>}
  </div>;
}

function UsageAnalysis({ report }: { report: ModelUsageReport }) {
  const { t } = useI18n();
  return <div className="asb-model-usage-analysis">
    <section className="asb-model-usage-distribution" aria-label={t("usage.composition.title")}>
      <ModelUsageDistributionChart items={composition(report, t)} ariaLabel={t("usage.composition.chartAria")}
        emptyMessage={t("usage.composition.empty")} />
    </section>
    <section className="asb-model-usage-trend" aria-label={t("usage.trend.title")}>
      <UsageTrendChart series={dailyTrend(report, t)} ariaLabel={t("usage.trend.chartAria")}
        title={t("usage.trend.title")} emptyMessage={t("usage.trend.empty")} valueKind="local-token" />
    </section>
  </div>;
}

export function UsagePage({ usage }: { usage: ReturnType<typeof useModelUsageReport> }) {
  const { t } = useI18n();
  const { read, loading, error } = usage;
  const report = read?.report ?? null;
  const hasData = report !== null && report.groups.length > 0;
  const notices = useMemo<InformationNotice[]>(() => (report?.issues ?? []).map((issue) => ({
    id: JSON.stringify([issue.app, issue.message]),
    kind: "warning", title: t("usage.issues.entry", { client: clientName(issue.app), message: issue.message }),
  })), [report, t]);
  useInformationSource(notices);
  return <section className="asb-panel asb-model-usage" aria-label={t("usage.consumption")}>
    <ModuleHeader title={t("usage.consumption")} />
    {error && <p className="asb-model-usage-notice" role="alert">{error}</p>}
    {loading && report === null ? <div className="asb-model-usage-loading" role="status" aria-label={t("usage.loading")}>
      <div className="asb-usage-stat-band">{[0, 1, 2, 3].map((key) => <div key={key} className="asb-skeleton" />)}</div>
      <div className="asb-skeleton asb-model-usage-loading-chart" />
    </div> : report && hasData ? <div className="asb-model-usage-content">
      <UsageTotals report={report} /><UsageAnalysis report={report} /><ModelUsageTable report={report} />
    </div> : report ? <><UsageTotals report={report} />
      <div className="asb-empty-state asb-model-usage-empty">
        <span className="asb-empty-state-icon" aria-hidden="true"><UsageIcon /></span>
        <h3 className="asb-section-title">{t("usage.empty")}</h3>
      </div></> : null}
  </section>;
}
