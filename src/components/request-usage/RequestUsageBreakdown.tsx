import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Table, type TableColumn } from "../Table";
import { Time } from "../Time";
import { RequestUsageBar } from "./RequestUsageBar";
import { RequestUsageQuality } from "./RequestUsageQuality";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageBreakdown({ report }: { report: RequestUsageReport }) {
  const { t, language } = useI18n();
  const m = report.summary;
  const sourceMaximum = Math.max(0, ...report.sources.map((source) => source.verifiedRecords));
  const timings = [
    { key: "usagePerformance.durationP50", value: m.durationP50Ms, samples: m.durationSamples },
    { key: "usagePerformance.durationP95", value: m.durationP95Ms, samples: m.durationSamples },
    { key: "usagePerformance.firstContentP50", value: m.firstContentP50Ms, samples: m.firstContentSamples },
  ] as const;
  const sourceColumns: Array<TableColumn<typeof report.sources[number]>> = [
    { key: "client", header: t("usagePerformance.clientColumn"), render: (source) => clientName(source.app) },
    { key: "records", header: t("usagePerformance.recordCountColumn"), align: "end",
      render: (source) => <div className="asb-request-source-count" data-client={source.app}>
        <strong>{number(source.verifiedRecords, language)}</strong>
        <RequestUsageBar value={source.verifiedRecords} maximum={sourceMaximum} />
      </div> },
    { key: "window", header: t("usagePerformance.window"), render: (source) => source.earliestAtMs !== null ? <>
      <Time iso={new Date(source.earliestAtMs).toISOString()} precision="second" />
      {source.latestAtMs !== null && <> — <Time iso={new Date(source.latestAtMs).toISOString()} precision="second" /></>}
    </> : t("usagePerformance.notRecorded") },
  ];
  return <section className="asb-request-usage-breakdown">
    <h3 className="asb-section-title">{t("usagePerformance.breakdown")}</h3>
    {m.records > 0 && <div className="asb-request-usage-breakdown-grid">
      <BarBlock title={t("usagePerformance.tokenFacts")} format={tokens}
        items={m.tokens.map((token) => ({ id: token.kind, label: t(`usagePerformance.token.${token.kind}`),
          value: token.tokens, note: t("usagePerformance.samples", { count: token.samples }) }))} />
      <BarBlock title={t("usagePerformance.timing")} format={duration}
        items={timings.map((item) => ({ id: item.key, label: t(item.key), value: item.value,
          note: t("usagePerformance.samples", { count: item.samples }) }))} />
      <RequestUsageQuality metrics={m} />
    </div>}
    <figure className="asb-request-usage-sources">
      <figcaption className="asb-group-title">{t("usagePerformance.sourceCoverage")}</figcaption>
      <Table ariaLabel={t("usagePerformance.sourceCoverage")} columns={sourceColumns}
        rows={report.sources} rowKey={(source) => source.app} />
    </figure>
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

function BarBlock({ title, items, format }: {
  title: string; items: BarItem[];
  format: (value: number | null, language: string) => string;
}) {
  const { language } = useI18n();
  const maximum = Math.max(0, ...items.map((item) => item.value ?? 0));
  return <figure className="asb-request-usage-breakdown-block">
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
