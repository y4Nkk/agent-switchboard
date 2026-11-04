import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { UsageTrendChart } from "../charts/UsageTrendChart";

export function RequestUsageTrends({ report }: { report: RequestUsageReport }) {
  const { t } = useI18n();
  const definitions = [
    { key: "speedP50", label: "usagePerformance.speedTrend", unit: "tok/s", scale: 1, kind: "generic" },
    { key: "firstOutputP50Ms", label: "usagePerformance.firstTrend", unit: "s", scale: 0.001, kind: "generic" },
    { key: "failureRate", label: "usagePerformance.failureTrend", unit: "%", scale: 1, kind: "percentage" },
  ] as const;
  return <div className="asb-request-usage-trends">
    {definitions.map((definition) => {
      const label = t(definition.label);
      return <UsageTrendChart key={definition.key} title={label} ariaLabel={label} size="compact"
        emptyMessage={t("usagePerformance.noSamples")} valueKind={definition.kind}
        series={[{ id: definition.key, label, unit: definition.unit,
          points: report.trend.flatMap((bucket) => {
            const value = bucket[definition.key];
            return value === null ? [] : [{ at: new Date(bucket.atMs).toISOString(), value: value * definition.scale }];
          }) }]} />;
    })}
  </div>;
}
