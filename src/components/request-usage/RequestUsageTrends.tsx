import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { Select } from "../Select";
import { UsageTrendChart } from "../charts/UsageTrendChart";

const METRICS = {
  speedP50: { label: "usagePerformance.trend.speed", title: "usagePerformance.speedTrend", unit: "tok/s", scale: 1, kind: "generic" },
  firstOutputP50Ms: { label: "usagePerformance.trend.first", title: "usagePerformance.firstTrend", unit: "s", scale: 0.001, kind: "generic" },
  failureRate: { label: "usagePerformance.trend.failures", title: "usagePerformance.failureTrend", unit: "%", scale: 1, kind: "percentage" },
} as const;
export type RequestUsageTrend = keyof typeof METRICS;

export function RequestUsageTrends({ report, metric, onMetricChange }: {
  report: RequestUsageReport; metric: RequestUsageTrend; onMetricChange: (metric: RequestUsageTrend) => void;
}) {
  const { t } = useI18n();
  const definition = METRICS[metric];
  const points = report.trend.flatMap((bucket) => {
    const value = bucket[metric];
    return value === null ? [] : [{ at: new Date(bucket.atMs).toISOString(), value: value * definition.scale }];
  });
  const title = t(definition.title);
  return <div className="asb-request-usage-trends">
    <div className="asb-request-usage-trend-controls">
      <Select value={metric} ariaLabel={t("usagePerformance.trendMetric")}
        options={Object.entries(METRICS).map(([value, entry]) => ({ value, label: t(entry.label) }))}
        onChange={(next) => onMetricChange(next as RequestUsageTrend)} />
      <p className="asb-scope-note">{t("usagePerformance.bucket", { minutes: report.bucketMs / 60_000 })}</p>
    </div>
    {points.length === 0 ? <p className="asb-request-usage-no-samples" role="status">{t("usagePerformance.noSamples")}</p>
      : <UsageTrendChart key={metric} title={title} ariaLabel={title} size="compact" valueKind={definition.kind}
        emptyMessage={t("usagePerformance.noSamples")} series={[{ id: metric, label: title, unit: definition.unit, points }]} />}
  </div>;
}
