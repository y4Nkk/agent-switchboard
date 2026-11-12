import { useId } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { Tabs } from "../Tabs";
import { Select } from "../Select";
import { chartMetricOptions, GROUP_METRICS, RECORD_METRICS, TREND_METRICS,
  type RequestUsageGroupMetric, type RequestUsageRecordMetric, type RequestUsageTrend } from "./chart-metrics";
import { RequestUsageGroups } from "./RequestUsageGroups";
import { RequestUsageRows } from "./RequestUsageRows";
import { RequestUsageTrends } from "./RequestUsageTrends";

export interface RequestUsageAnalysisSelection {
  view: "trend" | "models" | "requests";
  trend: RequestUsageTrend | null;
  models: RequestUsageGroupMetric | null;
  requests: RequestUsageRecordMetric | null;
}
export function RequestUsageAnalysis({ report, selection, onChange, onGroupPage, onPage }: {
  report: RequestUsageReport;
  selection: RequestUsageAnalysisSelection;
  onChange: (selection: RequestUsageAnalysisSelection) => void;
  onGroupPage: (page: number) => void;
  onPage: (page: number) => void;
}) {
  const { t } = useI18n();
  const id = useId();
  const views = ["trend", "models", "requests"] as const;
  const trendOptions = chartMetricOptions(TREND_METRICS, report.scope, t);
  const trend = selection.trend ?? trendOptions.find((option) =>
    report.trend.some((bucket) => bucket[option.value] !== null))?.value
    ?? (report.scope === "generation" ? "speedP50" : "durationP50Ms");
  const models = selection.models ?? "records";
  const requests = selection.requests ?? (report.scope === "generation" ? "outputTokens" : "durationMs");
  const options = {
    trend: trendOptions,
    models: chartMetricOptions(GROUP_METRICS, report.scope, t),
    requests: chartMetricOptions(RECORD_METRICS, report.scope, t),
  }[selection.view];
  return <div className="asb-request-usage-analysis">
    <h3 className="asb-section-title">{t("usagePerformance.analysis")}</h3>
    <div className="asb-request-usage-analysis-controls">
      <Tabs value={selection.view} scope={id} label={t("usagePerformance.analysis")}
        onChange={(view) => onChange({ ...selection, view })} tabs={views.map((view) => ({
          value: view, label: t(`usagePerformance.view.${view}`), controls: `${id}-${view}-panel`,
        }))} />
      <Select value={{ trend, models, requests }[selection.view]} options={options}
        ariaLabel={t("usagePerformance.chartMetric")} onChange={(value) => {
          if (selection.view === "trend") onChange({ ...selection, trend: value as RequestUsageTrend });
          else if (selection.view === "models") onChange({ ...selection, models: value as RequestUsageGroupMetric });
          else onChange({ ...selection, requests: value as RequestUsageRecordMetric });
        }} />
      {selection.view === "trend" && <span className="asb-request-usage-caption">
        {t("usagePerformance.bucket", { minutes: report.bucketMs / 60_000 })}
      </span>}
    </div>
    {views.map((view) => <div key={view} id={`${id}-${view}-panel`} role="tabpanel"
      aria-labelledby={`${id}-${view}-tab`} hidden={selection.view !== view}>
      {selection.view === view && (view === "trend"
        ? <RequestUsageTrends report={report} metric={trend} />
        : view === "models" ? <RequestUsageGroups report={report} metric={models} onPage={onGroupPage} />
          : <RequestUsageRows report={report} metric={requests} onPage={onPage} />)}
    </div>)}
  </div>;
}
