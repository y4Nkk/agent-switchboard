import { useState } from "react";
import type { RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { cost, duration, tokens } from "./format";

export function RequestUsageDetails({ metrics: m }: { metrics: RequestUsageMetrics }) {
  const { t, language } = useI18n();
  const [open, setOpen] = useState(false);
  const attemptsKnown = m.attemptSamples > 0;
  return <CollapsibleSection open={open} onOpenChange={setOpen} quiet
    label={t("usagePerformance.moreStats")} value={t("usagePerformance.moreStatsHint")}>
    <div className="asb-request-usage-detail-groups">
      <TokenFacts metrics={m} />
      <section className="asb-request-usage-facts">
        <h4 className="asb-group-title">{t("usagePerformance.executionFacts")}</h4>
        <FactList rows={[
          { label: t("usagePerformance.duration"), value: m.durationSamples === 0 ? t("usagePerformance.notRecorded") : <>
            {duration(m.durationP50Ms, language)} / {duration(m.durationP95Ms, language)}
            <span className="asb-request-usage-caption">{t("usagePerformance.samples", { count: m.durationSamples })}</span></> },
          { label: t("usagePerformance.cost"), value: m.costSamples === 0 ? t("usagePerformance.notRecorded") : <>
            {cost(m.estimatedCostUsd, language)}
            <span className="asb-request-usage-caption">{t("usagePerformance.samples", { count: m.costSamples })}</span></> },
          { label: t("usagePerformance.pricingErrors"), value: tokens(m.pricingErrors) },
          { label: t("usagePerformance.retryRequests"), value: attemptsKnown ? tokens(m.retryRequests) : t("usagePerformance.notRecorded") },
          { label: t("usagePerformance.retryCount"), value: attemptsKnown ? tokens(m.retries) : t("usagePerformance.notRecorded") },
          { label: t("usagePerformance.recovered"), value: attemptsKnown ? tokens(m.recoveredRequests) : t("usagePerformance.notRecorded") },
          { label: t("usagePerformance.attemptCoverage"), value: `${tokens(m.attemptSamples)} / ${tokens(m.requests)}` },
        ]} />
      </section>
      <section className="asb-request-usage-facts">
        <h4 className="asb-group-title">{t("usagePerformance.failures")}</h4>
        <FactList rows={[
          { label: t("usagePerformance.partialOutput"), value: tokens(m.incomplete) },
          { label: t("usagePerformance.rateLimited"), value: tokens(m.rateLimited) },
          { label: t("usagePerformance.serverErrors"), value: tokens(m.upstreamErrors) },
          { label: t("usagePerformance.unclassified"), value: tokens(m.unclassifiedFailures) },
          { label: t("usagePerformance.otherFailures"), value: tokens(m.failed - m.incomplete - m.rateLimited - m.upstreamErrors - m.unclassifiedFailures) },
          { label: t("usagePerformance.outcome.disconnected"), value: tokens(m.disconnected) },
        ]} />
      </section>
    </div>
  </CollapsibleSection>;
}

function TokenFacts({ metrics }: { metrics: RequestUsageMetrics }) {
  const { t } = useI18n();
  return <section className="asb-request-usage-facts">
    <h4 className="asb-group-title">{t("usagePerformance.tokenFacts")}</h4>
    <FactList rows={metrics.tokens.map((item) => ({ label: t(`usagePerformance.token.${item.kind}`),
      value: item.tokens === null ? t("usagePerformance.notRecorded") : <>
        {tokens(item.tokens)} <span className="asb-request-usage-caption">tokens · {t("usagePerformance.samples", { count: item.samples })}</span>
      </>,
    }))} />
  </section>;
}
