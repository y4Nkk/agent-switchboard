import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { UsageTrendChart } from "../charts/UsageTrendChart";
import { TREND_METRICS, type RequestUsageTrend } from "./chart-metrics";

export function RequestUsageTrends({ report, metric }: {
  report: RequestUsageReport; metric: RequestUsageTrend;
}) {
  const { t } = useI18n();
  const definition = TREND_METRICS[metric];
  const points = report.trend.flatMap((bucket) => {
    const value = bucket[metric];
    return value === null ? [] : [{ at: new Date(bucket.atMs).toISOString(), value: value * definition.scale }];
  });
  const label = t(definition.labelKey);
  return <div className="asb-request-usage-trends">
    {points.length === 0 ? <p className="asb-request-usage-no-samples" role="status">{t("usagePerformance.noSamples")}</p>
      : <UsageTrendChart key={metric} ariaLabel={label} size="compact" valueKind={definition.kind}
        emptyMessage={t("usagePerformance.noSamples")} series={[{ id: metric, label, unit: definition.unit, points }]} />}
  </div>;
}
