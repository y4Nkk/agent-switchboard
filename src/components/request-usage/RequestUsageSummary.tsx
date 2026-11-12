import type { ReactNode } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { RequestUsageRatioRing, RequestUsageSparkline } from "./RequestUsageMicrocharts";
import { requestTrendPoints } from "./request-trend";
import { duration, number, percent } from "./format";

export function RequestUsageSummary({ report }: { report: RequestUsageReport }) {
  const { t, language } = useI18n();
  const m = report.summary;
  const generation = report.scope === "generation";
  const trend = (key: "speedP50" | "firstTokenP50Ms" | "durationP50Ms") => {
    const points = requestTrendPoints(report, key);
    return points.some((point) => point.value !== null) ? <div className="asb-request-metric-trend">
      <RequestUsageSparkline points={points} />
      <span className="asb-request-usage-caption">{t("usagePerformance.summaryTrend")}</span>
    </div> : null;
  };
  return <div className="asb-request-metric-grid">
    <MetricCard label={t(generation ? "usagePerformance.speed" : "usagePerformance.durationP50")}
      value={generation ? number(m.outputRateP50, language) : duration(m.durationP50Ms, language)}
      unit={generation ? "tok/s" : undefined} chart={trend(generation ? "speedP50" : "durationP50Ms")}
      detail={generation ? m.speedSamples > 0 ? t("usagePerformance.speedDetail", {
        p10: number(m.outputRateP10, language), count: m.speedSamples }) : t("usagePerformance.noSamples")
        : t("usagePerformance.samples", { count: m.durationSamples })} />
    <MetricCard label={t(generation ? "usagePerformance.firstToken" : "usagePerformance.durationP95")}
      value={duration(generation ? m.firstTokenP50Ms : m.durationP95Ms, language)}
      chart={generation ? trend("firstTokenP50Ms") : undefined}
      detail={generation ? m.firstTokenSamples > 0 ? t("usagePerformance.firstDetail", {
        p95: duration(m.firstTokenP95Ms, language), count: m.firstTokenSamples }) : t("usagePerformance.noSamples")
        : t("usagePerformance.samples", { count: m.durationSamples })} />
    <MetricCard label={t("usagePerformance.successRatio")} value={percent(m.successRatio, language)}
      ring={<RequestUsageRatioRing value={m.successRatio} />}
      detail={t("usagePerformance.successDetail", { complete: m.succeeded, failed: m.failed, unknown: m.unknown })} />
    {generation ? <MetricCard label={t("usagePerformance.cache")} value={percent(m.cacheHitRate, language)}
      ring={<RequestUsageRatioRing value={m.cacheHitRate} />}
      detail={m.cacheSamples > 0 ? t("usagePerformance.samples", { count: m.cacheSamples }) : t("usagePerformance.notRecorded")} />
      : <MetricCard label={t("usagePerformance.recordsCount")} value={number(m.records, language)} />}
  </div>;
}

function MetricCard({ label, value, unit, detail, chart, ring }: {
  label: string; value: string; unit?: string; detail?: string; chart?: ReactNode; ring?: ReactNode;
}) {
  return <section className="asb-request-metric-card">
    <div className="asb-request-metric-head">
      <div className="asb-request-metric-readout">
        <h3 className="asb-usage-stat-label">{label}</h3>
        <div className="asb-usage-stat-value-row">
          <p className="asb-usage-stat-value">{value}</p>
          {unit && <span className="asb-usage-stat-unit">{unit}</span>}
        </div>
      </div>
      {ring}
    </div>
    {chart}
    {detail && <p className="asb-usage-stat-detail">{detail}</p>}
  </section>;
}
