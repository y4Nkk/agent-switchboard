import { useId, type CSSProperties } from "react";
import type { RequestUsageReport } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Table, type TableColumn } from "../Table";
import { Time } from "../Time";
import { ModuleHeader } from "../WorkspaceHeader";
import { RequestUsagePoints } from "./RequestUsagePoints";
import { RequestUsageQuality } from "./RequestUsageQuality";
import { cost, duration, number, tokens } from "./format";

export function RequestUsageBreakdown({ report }: { report: RequestUsageReport }) {
  const { t, language } = useI18n();
  const id = useId();
  const m = report.summary;
  const timings = [
    { key: "usagePerformance.durationP50", value: m.durationP50Ms, samples: m.durationSamples },
    { key: "usagePerformance.durationP95", value: m.durationP95Ms, samples: m.durationSamples },
    { key: "usagePerformance.firstContentP50", value: m.firstContentP50Ms, samples: m.firstContentSamples },
  ] as const;
  return <>
    {m.records > 0 && <section className="asb-usage-surface asb-request-usage-breakdown" aria-labelledby={id}>
      <ModuleHeader id={id} title={t("usagePerformance.breakdown")} />
      <div className="asb-request-usage-breakdown-grid">
        <ComparisonPlot title={t("usagePerformance.tokenFacts")} format={tokens}
          items={m.tokens.map((token) => ({ id: token.kind, label: t(`usagePerformance.token.${token.kind}`),
            value: token.tokens, note: t("usagePerformance.samples", { count: token.samples }) }))} />
        <ComparisonPlot title={t("usagePerformance.timing")} format={duration}
          items={timings.map((item) => ({ id: item.key, label: t(item.key), value: item.value,
            note: t("usagePerformance.samples", { count: item.samples }) }))} />
        <RequestUsageQuality metrics={m} />
      </div>
      {m.estimatedCostUsd !== null && <p className="asb-request-usage-status">
        {t("usagePerformance.cost")} {cost(m.estimatedCostUsd, language)}
        {" · "}{t("usagePerformance.samples", { count: m.costSamples })}
      </p>}
    </section>}
    <RequestUsageSources report={report} />
  </>;
}

function RequestUsageSources({ report }: { report: RequestUsageReport }) {
  const { t, language } = useI18n();
  const heading = useId();
  const { sources, startAtMs, endAtMs } = report;
  const sourceMaximum = Math.max(0, ...sources.map((source) => source.verifiedRecords));
  const label = t("usagePerformance.sourceCoverage");
  const sourceColumns: Array<TableColumn<typeof sources[number]>> = [
    { key: "client", header: t("usagePerformance.clientColumn"), render: (source) => clientName(source.app) },
    { key: "records", header: t("usagePerformance.recordCountColumn"), align: "end",
      render: (source) => <div className="asb-request-source-count" data-client={source.app}>
        <strong>{number(source.verifiedRecords, language)}</strong>
        <RequestUsagePoints value={source.verifiedRecords} maximum={sourceMaximum} />
      </div> },
    { key: "window", header: t("usagePerformance.window"), render: (source) => source.earliestAtMs !== null ? <>
      <Time iso={new Date(source.earliestAtMs).toISOString()} precision="second" />
      {source.latestAtMs !== null && <> — <Time iso={new Date(source.latestAtMs).toISOString()} precision="second" /></>}
    </> : t("usagePerformance.notRecorded") },
  ];
  return <section className="asb-usage-surface" aria-labelledby={heading}>
    <ModuleHeader id={heading} title={label} primary={<span className="asb-request-usage-caption">
      {t("usagePerformance.reportWindow")} <Time iso={new Date(startAtMs).toISOString()} precision="second" />
      {" — "}<Time iso={new Date(endAtMs).toISOString()} precision="second" />
    </span>} />
    <Table ariaLabel={label} columns={sourceColumns} rows={sources} rowKey={(source) => source.app} />
  </section>;
}

interface ComparisonItem { id: string; label: string; value: number | null; note: string }

function ComparisonPlot({ title, items, format }: {
  title: string; items: ComparisonItem[];
  format: (value: number | null, language: string) => string;
}) {
  const { language, t } = useI18n();
  const maximum = Math.max(0, ...items.map((item) => item.value ?? 0));
  return <figure className="asb-request-usage-breakdown-block asb-request-comparison">
    <figcaption className="asb-group-title">{title}</figcaption>
    {items.length === 0 ? <p className="asb-request-usage-caption">{t("usagePerformance.noSamples")}</p>
      : <div className="asb-request-comparison-plot">
        <div className="asb-request-comparison-axis" aria-hidden="true">
          {maximum > 0 && <span>{format(maximum, language)}</span>}
          <span>0</span>
        </div>
        <ul className="asb-request-comparison-series" style={{ "--asb-request-comparison-columns": items.length } as CSSProperties}>
          {items.map((item) => <li key={item.id}>
            <RequestUsagePoints value={item.value} maximum={maximum} />
            <div className="asb-request-comparison-readout">
              <span>{item.label}</span>
              <strong>{format(item.value, language)}</strong>
              <span className="asb-request-usage-caption">{item.note}</span>
            </div>
          </li>)}
        </ul>
      </div>}
  </figure>;
}
