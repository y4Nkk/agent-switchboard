import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { UsageTrendChart } from "../charts/UsageTrendChart";
import { TREND_METRICS, type RequestUsageTrend } from "./chart-metrics";
import { requestTrendPoints } from "./request-trend";

export function RequestUsageTrends({ report, metric }: {
  report: RequestUsageReport; metric: RequestUsageTrend;
}) {
  const { t } = useI18n();
  const definition = TREND_METRICS[metric];
  const points = requestTrendPoints(report, metric).map((point) => ({
    at: new Date(point.atMs).toISOString(), value: point.value === null ? null : point.value * definition.scale,
  }));
  const label = t(definition.labelKey);
  return <div className="asb-request-usage-trends">
    {!points.some((point) => point.value !== null) ? <p className="asb-request-usage-no-samples" role="status">{t("usagePerformance.noSamples")}</p>
      : <UsageTrendChart key={metric} ariaLabel={label} size="compact" valueKind={definition.kind}
        emptyMessage={t("usagePerformance.noSamples")} series={[{ id: metric, label, unit: definition.unit, points }]} />}
  </div>;
}
