import type { RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { StatCards } from "../charts/stat-cards";
import { duration, number, percent } from "./format";

export function RequestUsageSummary({ metrics: m }: { metrics: RequestUsageMetrics }) {
  const { t, language } = useI18n();
  return <div className="asb-request-usage-summary">
    <StatCards stats={[
      { label: t("usagePerformance.speed"), value: number(m.outputRateP50, language), unit: "tok/s",
        detail: m.speedSamples > 0 ? t("usagePerformance.speedDetail", {
          p10: number(m.outputRateP10, language), count: m.speedSamples }) : t("usagePerformance.noSamples") },
      { label: t("usagePerformance.firstOutput"), value: duration(m.firstOutputP50Ms, language),
        detail: m.firstOutputSamples > 0 ? t("usagePerformance.firstDetail", {
          p95: duration(m.firstOutputP95Ms, language), count: m.firstOutputSamples }) : t("usagePerformance.noSamples") },
      { label: t("usagePerformance.completion"), value: percent(m.completionRate, language),
        detail: t("usagePerformance.completionDetail", { complete: m.completed, failed: m.failed, disconnected: m.disconnected }) },
      { label: t("usagePerformance.cache"), value: percent(m.cacheHitRate, language),
        detail: m.cacheSamples > 0 ? t("usagePerformance.samples", { count: m.cacheSamples }) : t("usagePerformance.notRecorded") },
    ]} />
    {m.speedSamples > 0 && m.speedSamples < 20 && <p className="asb-scope-note">{t("usagePerformance.smallSample")}</p>}
  </div>;
}
