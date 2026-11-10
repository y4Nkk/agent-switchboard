import { useState } from "react";
import type { RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageDetails({ metrics: m }: { metrics: RequestUsageMetrics }) {
  const { t, language } = useI18n();
  const [open, setOpen] = useState(false);
  const covered = (value: string, samples: number) => `${value} · n=${samples}`;
  return <CollapsibleSection open={open} onOpenChange={setOpen} quiet label={t("usagePerformance.moreStats")}>
    <div className="asb-request-usage-detail-groups">
      <section className="asb-request-usage-facts">
        <h4 className="asb-group-title">{t("usagePerformance.tokenFacts")}</h4>
        <FactList rows={m.tokens.map((token) => ({ label: t(`usagePerformance.token.${token.kind}`),
          value: covered(tokens(token.tokens), token.samples) }))} />
      </section>
      <section className="asb-request-usage-facts">
        <h4 className="asb-group-title">{t("usagePerformance.executionFacts")}</h4>
        <FactList rows={[
          { label: t("usagePerformance.duration"), value: covered(`${duration(m.durationP50Ms, language)} / ${duration(m.durationP95Ms, language)}`, m.durationSamples) },
          { label: t("usagePerformance.firstContentP50"), value: covered(duration(m.firstContentP50Ms, language), m.firstContentSamples) },
          { label: t("usagePerformance.cost"), value: covered(cost(m.estimatedCostUsd, language), m.costSamples) },
          { label: t("usagePerformance.retryRecords"), value: number(m.attemptSamples ? m.retryRecords : null, language) },
          { label: t("usagePerformance.totalRetries"), value: number(m.attemptSamples ? m.retries : null, language) },
          { label: t("usagePerformance.recovered"), value: number(m.attemptSamples ? m.recoveredRecords : null, language) },
          { label: t("usagePerformance.attemptCoverage"), value: number(m.attemptSamples, language) },
        ]} />
      </section>
      <section className="asb-request-usage-facts">
        <h4 className="asb-group-title">{t("usagePerformance.failures")}</h4>
        <FactList rows={[
          { label: t("usagePerformance.rateLimited"), value: number(m.rateLimited, language) },
          { label: t("usagePerformance.serverErrors"), value: number(m.upstreamErrors, language) },
          { label: t("usagePerformance.unclassified"), value: number(m.unclassifiedFailures, language) },
          { label: t("usagePerformance.otherFailures"), value: number(m.failed - m.rateLimited - m.upstreamErrors - m.unclassifiedFailures, language) },
        ]} />
      </section>
    </div>
  </CollapsibleSection>;
}
