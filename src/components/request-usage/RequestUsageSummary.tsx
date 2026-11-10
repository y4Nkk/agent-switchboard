import type { MeasurementScope, RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { StatCards } from "../charts/stat-cards";
import { duration, number, percent } from "./format";

export function RequestUsageSummary({ metrics: m, scope }: { metrics: RequestUsageMetrics; scope: MeasurementScope }) {
  const { t, language } = useI18n();
  return <div className="asb-request-usage-summary">
    {m.mixedMeasurements && <p className="asb-scope-note">{t("usagePerformance.mixedMeasurements")}</p>}
    <StatCards stats={[
      ...(scope === "generation" ? [{ label: t("usagePerformance.speed"), value: number(m.outputRateP50, language), unit: "tok/s",
        detail: m.speedSamples > 0 ? t("usagePerformance.speedDetail", {
          p10: number(m.outputRateP10, language), count: m.speedSamples }) : t("usagePerformance.noSamples") },
      { label: t("usagePerformance.firstToken"), value: duration(m.firstTokenP50Ms, language),
        detail: m.firstTokenSamples > 0 ? t("usagePerformance.firstDetail", {
          p95: duration(m.firstTokenP95Ms, language), count: m.firstTokenSamples }) : t("usagePerformance.noSamples") }]
        : [{ label: t("usagePerformance.durationP50"), value: duration(m.durationP50Ms, language),
          detail: t("usagePerformance.samples", { count: m.durationSamples }) },
          { label: t("usagePerformance.durationP95"), value: duration(m.durationP95Ms, language),
            detail: t("usagePerformance.samples", { count: m.durationSamples }) }]),
      { label: t("usagePerformance.successRatio"), value: percent(m.successRatio, language),
        detail: t("usagePerformance.successDetail", { complete: m.succeeded, failed: m.failed, unknown: m.unknown }) },
      scope === "generation" ? { label: t("usagePerformance.cache"), value: percent(m.cacheHitRate, language),
        detail: m.cacheSamples > 0 ? t("usagePerformance.samples", { count: m.cacheSamples }) : t("usagePerformance.notRecorded") }
        : { label: t("usagePerformance.recordsCount"), value: number(m.records, language) },
    ]} />
    {m.speedSamples > 0 && m.speedSamples < 20 && <p className="asb-scope-note">{t("usagePerformance.smallSample")}</p>}
  </div>;
}
