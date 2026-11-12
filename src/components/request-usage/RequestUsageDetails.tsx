import { useState, type ReactNode } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { CollapsibleSection } from "../Collapsible";
import { Time } from "../Time";
import { RequestUsageBar } from "./RequestUsageBar";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageDetails({ report }: { report: RequestUsageReport }) {
  const { t, language } = useI18n();
  const [open, setOpen] = useState(false);
  const m = report.summary;
  const attemptsKnown = m.attemptSamples > 0;
  const timings = [
    { key: "usagePerformance.durationP50", value: m.durationP50Ms, samples: m.durationSamples },
    { key: "usagePerformance.durationP95", value: m.durationP95Ms, samples: m.durationSamples },
    { key: "usagePerformance.firstContentP50", value: m.firstContentP50Ms, samples: m.firstContentSamples },
  ] as const;
  const failures = [
    { key: "usagePerformance.rateLimited", value: m.rateLimited },
    { key: "usagePerformance.serverErrors", value: m.upstreamErrors },
    { key: "usagePerformance.unclassified", value: m.unclassifiedFailures },
    { key: "usagePerformance.otherFailures", value: m.failed - m.rateLimited - m.upstreamErrors - m.unclassifiedFailures },
  ] as const;
  const retries = [
    { key: "usagePerformance.retryRecords", value: m.retryRecords },
    { key: "usagePerformance.totalRetries", value: m.retries },
    { key: "usagePerformance.recovered", value: m.recoveredRecords },
    { key: "usagePerformance.attemptCoverage", value: m.attemptSamples },
  ] as const;
  return <CollapsibleSection open={open} onOpenChange={setOpen} quiet label={t("usagePerformance.moreStats")}>
    <div className="asb-request-usage-extra-charts">
      {m.records > 0 && <>
        <MetricBars title={t("usagePerformance.tokenFacts")} format={tokens}
          items={m.tokens.map((token) => ({ id: token.kind, label: t(`usagePerformance.token.${token.kind}`),
            value: token.tokens, note: t("usagePerformance.samples", { count: token.samples }) }))} />
        <MetricBars title={t("usagePerformance.timing")} format={duration}
          items={timings.map((item) => ({ id: item.key, label: t(item.key), value: item.value,
            note: t("usagePerformance.samples", { count: item.samples }) }))} />
        <MetricBars title={t("usagePerformance.failures")} format={number} tone="failed"
          items={failures.map((item) => ({ id: item.key, label: t(item.key), value: item.value }))} />
        <MetricBars title={t("usagePerformance.retries")} format={number}
          items={retries.map((item) => ({ id: item.key, label: t(item.key), value: attemptsKnown ? item.value : null }))} />
      </>}
      <MetricBars title={t("usagePerformance.sourceCoverage")} format={number}
        items={report.sources.map((source) => ({ id: source.app, label: clientName(source.app), value: source.verifiedRecords,
          note: source.earliestAtMs !== null && <>
            <Time iso={new Date(source.earliestAtMs).toISOString()} precision="second" />
            {source.latestAtMs !== null && <> — <Time iso={new Date(source.latestAtMs).toISOString()} precision="second" /></>}
          </> }))} />
    </div>
    <div className="asb-request-usage-extra-status">
      {m.estimatedCostUsd !== null && <span>{t("usagePerformance.cost")} {cost(m.estimatedCostUsd, language)}
        {" · "}{t("usagePerformance.samples", { count: m.costSamples })}</span>}
      <span>{t("usagePerformance.window")}{" "}
        <Time iso={new Date(report.startAtMs).toISOString()} precision="second" /> — <Time iso={new Date(report.endAtMs).toISOString()} precision="second" />
      </span>
    </div>
  </CollapsibleSection>;
}

interface MetricItem { id: string; label: string; value: number | null; note?: ReactNode }

function MetricBars({ title, items, format, tone }: {
  title: string; items: MetricItem[]; tone?: "failed";
  format: (value: number | null, language: string) => string;
}) {
  const { language } = useI18n();
  if (items.every((item) => item.value === null)) return null;
  const maximum = Math.max(0, ...items.map((item) => item.value ?? 0));
  return <figure className="asb-request-usage-mini-plot" aria-label={title} data-outcome={tone}>
    <figcaption className="asb-group-title">{title}</figcaption>
    <ul>
      {items.map((item) => <li key={item.id}>
        <div className="asb-request-usage-mini-label"><span>{item.label}</span><strong>{format(item.value, language)}</strong></div>
        <RequestUsageBar value={item.value} maximum={maximum} />
        {item.note && <span className="asb-request-usage-caption">{item.note}</span>}
      </li>)}
    </ul>
  </figure>;
}
