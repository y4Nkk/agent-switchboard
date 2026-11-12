import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n/messages";
import { clientName } from "../../lib/client-name";
import { Table, type TableColumn } from "../Table";
import { Time } from "../Time";
import { RequestUsageBar } from "./RequestUsageBar";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageBreakdown({ report }: { report: RequestUsageReport }) {
  const { t, language } = useI18n();
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
  const retryColumns: Array<TableColumn<{ key: MessageKey; value: number }>> = [
    { key: "fact", header: t("usagePerformance.metricColumn"), render: (row) => t(row.key) },
    { key: "value", header: t("usagePerformance.countColumn"), align: "end", render: (row) => number(row.value, language) },
  ];
  const sourceColumns: Array<TableColumn<typeof report.sources[number]>> = [
    { key: "client", header: t("usagePerformance.clientColumn"), render: (source) => clientName(source.app) },
    { key: "records", header: t("usagePerformance.recordCountColumn"), align: "end",
      render: (source) => number(source.verifiedRecords, language) },
    { key: "window", header: t("usagePerformance.window"), render: (source) => source.earliestAtMs !== null && <>
      <Time iso={new Date(source.earliestAtMs).toISOString()} precision="second" />
      {source.latestAtMs !== null && <> — <Time iso={new Date(source.latestAtMs).toISOString()} precision="second" /></>}
    </> },
  ];
  return <section className="asb-request-usage-breakdown">
    <h3 className="asb-group-title">{t("usagePerformance.moreStats")}</h3>
    <div className="asb-request-usage-breakdown-grid">
      {m.records > 0 && <>
        <BarBlock title={t("usagePerformance.tokenFacts")} format={tokens}
          items={m.tokens.map((token) => ({ id: token.kind, label: t(`usagePerformance.token.${token.kind}`),
            value: token.tokens, note: t("usagePerformance.samples", { count: token.samples }) }))} />
        <BarBlock title={t("usagePerformance.timing")} format={duration}
          items={timings.map((item) => ({ id: item.key, label: t(item.key), value: item.value,
            note: t("usagePerformance.samples", { count: item.samples }) }))} />
        <BarBlock title={t("usagePerformance.failures")} format={number} tone="failed"
          items={failures.map((item) => ({ id: item.key, label: t(item.key), value: item.value }))} />
        {attemptsKnown && <figure className="asb-request-usage-breakdown-block">
          <figcaption className="asb-group-title">{t("usagePerformance.retries")}</figcaption>
          <Table ariaLabel={t("usagePerformance.retries")} columns={retryColumns}
            rows={retries.map((item) => ({ key: item.key, value: item.value }))}
            rowKey={(row) => row.key} />
        </figure>}
      </>}
      <figure className="asb-request-usage-breakdown-block">
        <figcaption className="asb-group-title">{t("usagePerformance.sourceCoverage")}</figcaption>
        <Table ariaLabel={t("usagePerformance.sourceCoverage")} columns={sourceColumns}
          rows={report.sources} rowKey={(source) => source.app} />
      </figure>
    </div>
    <div className="asb-request-usage-breakdown-status">
      {m.estimatedCostUsd !== null && <span>{t("usagePerformance.cost")} {cost(m.estimatedCostUsd, language)}
        {" · "}{t("usagePerformance.samples", { count: m.costSamples })}</span>}
      <span>{t("usagePerformance.window")}{" "}
        <Time iso={new Date(report.startAtMs).toISOString()} precision="second" /> — <Time iso={new Date(report.endAtMs).toISOString()} precision="second" />
      </span>
    </div>
  </section>;
}

interface BarItem { id: string; label: string; value: number | null; note?: string }

function BarBlock({ title, items, format, tone }: {
  title: string; items: BarItem[]; tone?: "failed";
  format: (value: number | null, language: string) => string;
}) {
  const { language } = useI18n();
  if (items.every((item) => item.value === null)) return null;
  const maximum = Math.max(0, ...items.map((item) => item.value ?? 0));
  return <figure className="asb-request-usage-breakdown-block" data-outcome={tone}>
    <figcaption className="asb-group-title">{title}</figcaption>
    <ul>
      {items.map((item) => <li key={item.id}>
        <div className="asb-request-usage-breakdown-row"><span>{item.label}</span><strong>{format(item.value, language)}</strong></div>
        <RequestUsageBar value={item.value} maximum={maximum} />
        {item.note && <span className="asb-request-usage-caption">{item.note}</span>}
      </li>)}
    </ul>
  </figure>;
}
