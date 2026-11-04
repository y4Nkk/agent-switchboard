import type { RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { StatCards } from "../charts/stat-cards";
import { FactList } from "../FactList";
import { cost, duration, number, percent, tokens } from "./format";

export function RequestUsageSummary({ metrics: m }: { metrics: RequestUsageMetrics }) {
  const { t, language } = useI18n();
  return <div className="asb-request-usage-summary">
    <StatCards stats={[
      { label: t("usagePerformance.speed"), value: number(m.outputRateP50, language), unit: "tok/s",
        detail: t("usagePerformance.speedDetail", { p10: number(m.outputRateP10, language), count: m.speedSamples }) },
      { label: t("usagePerformance.firstOutput"), value: duration(m.firstOutputP50Ms, language),
        detail: t("usagePerformance.firstDetail", { p95: duration(m.firstOutputP95Ms, language), count: m.firstOutputSamples }) },
      { label: t("usagePerformance.completion"), value: percent(m.completionRate, language),
        detail: t("usagePerformance.completionDetail", { complete: m.completed, failed: m.failed, disconnected: m.disconnected }) },
      { label: t("usagePerformance.cache"), value: percent(m.cacheHitRate, language),
        detail: t("usagePerformance.samples", { count: m.cacheSamples }) },
    ]} />
    <FactList inline rows={[
      { label: t("usagePerformance.requests"), value: tokens(m.requests) },
      { label: t("usagePerformance.duration"), value: `${duration(m.durationP50Ms, language)} · P95 ${duration(m.durationP95Ms, language)} · n=${m.durationSamples}` },
      { label: t("usagePerformance.retries"), value: m.attemptSamples === 0 ? "—" : t("usagePerformance.retrySummary", {
        requests: m.retryRequests, retries: m.retries, recovered: m.recoveredRequests, count: m.attemptSamples }) },
      { label: t("usagePerformance.failures"), value: t("usagePerformance.failureSummary", {
        limited: m.rateLimited, upstream: m.upstreamErrors, unknown: m.unclassifiedFailures,
        incomplete: m.incomplete, other: m.failed - m.incomplete - m.rateLimited - m.upstreamErrors - m.unclassifiedFailures }) },
      { label: t("usagePerformance.cost"), value: `${cost(m.estimatedCostUsd, language)} · ${t("usagePerformance.samples", { count: m.costSamples })}` },
      { label: t("usagePerformance.pricingErrors"), value: tokens(m.pricingErrors) },
    ]} />
    <div className="asb-request-usage-tokens">
      <FactList inline rows={m.tokens.map((item) => ({ label: t(`usagePerformance.token.${item.kind}`),
        value: <span>{tokens(item.tokens)} <span className="asb-request-usage-caption">tokens · {t("usagePerformance.samples", { count: item.samples })}</span></span> }))} />
    </div>
  </div>;
}
