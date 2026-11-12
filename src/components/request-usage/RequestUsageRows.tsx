import { useId, useState } from "react";
import type { RequestUsageReport, RequestUsageRow } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Collapsible, CollapsibleChevron } from "../Collapsible";
import { FactList } from "../FactList";
import { Pagination } from "../Pagination";
import { Time } from "../Time";
import { RECORD_METRICS, type RequestUsageRecordMetric } from "./chart-metrics";
import { RequestUsageBar, RequestUsageBarScale } from "./RequestUsageBar";
import { cost, duration, number, speed, tokens } from "./format";

export function RequestUsageRows({ report, metric, onPage }: {
  report: RequestUsageReport; metric: RequestUsageRecordMetric; onPage: (page: number) => void;
}) {
  const { t, language } = useI18n();
  const id = useId();
  const [expanded, setExpanded] = useState<string | null>(null);
  const definition = RECORD_METRICS[metric];
  const maximum = Math.max(0, ...report.rows.map((row) => row[metric] ?? 0));
  return <div className="asb-request-usage-section">
    <figure className="asb-request-usage-plot" aria-label={`${t("usagePerformance.records")} · ${t(definition.labelKey)}`}>
      <RequestUsageBarScale end={maximum > 0 ? definition.format(maximum, language) : null} />
      <ul className="asb-request-usage-bars">
        {report.rows.map((row) => {
          const open = expanded === row.id;
          const detailsId = `${id}-${row.id}`;
          return <li key={row.id}>
            <button type="button" className="asb-request-usage-bar-line asb-request-usage-record"
              data-outcome={row.outcome} aria-expanded={open} aria-controls={open ? detailsId : undefined}
              onClick={() => setExpanded(open ? null : row.id)}>
              <span className="asb-request-usage-bar-label">
                <span className="asb-code">{row.model ?? t("usage.model.unrecorded")}</span>
                <span className="asb-request-usage-caption">
                  <Time iso={new Date(row.atMs).toISOString()} precision="second" /> · {clientName(row.app)}
                </span>
              </span>
              <RequestUsageBar value={row[metric]} maximum={maximum} />
              <span className="asb-request-usage-bar-value">
                <span className="asb-request-usage-record-value">
                  <strong>{definition.format(row[metric], language)}</strong><CollapsibleChevron open={open} />
                </span>
                <span className={`asb-request-outcome is-${row.outcome}`}>
                  {t(row.eventKind === "response.incomplete" ? "usagePerformance.incomplete" : `usagePerformance.outcome.${row.outcome}`)}
                  {row.status !== null && ` · ${row.status}`}
                </span>
              </span>
            </button>
            <Collapsible open={open} id={detailsId}><RequestDetails row={row} /></Collapsible>
          </li>;
        })}
      </ul>
    </figure>
    <Pagination total={report.summary.records} page={report.page} pageSize={report.pageSize}
      onPageChange={(page) => { setExpanded(null); onPage(page); }} label={t("usagePerformance.recordPages")} />
  </div>;
}

function RequestDetails({ row }: { row: RequestUsageRow }) {
  const { t, language } = useI18n();
  const timings = [
    ["usagePerformance.durationColumn", row.durationMs],
    ["usagePerformance.firstColumn", row.firstTokenMs],
    ["usagePerformance.firstContent", row.firstContentMs],
  ] as const;
  const tokenValues = [
    ["usagePerformance.inputTokens", row.inputTokens],
    ["usagePerformance.outputTokens", row.outputTokens],
    ["usagePerformance.token.cacheRead", row.cacheReadTokens],
    ["usagePerformance.token.cacheCreation", row.cacheCreationTokens],
    ["usagePerformance.token.reasoning", row.reasoningTokens],
  ] as const;
  const facts = [
    { label: t("usagePerformance.eventName"), value: row.eventName },
    { label: t("usagePerformance.eventKind"), value: row.eventKind },
    { label: t("usagePerformance.timeSource"), value: t(`usagePerformance.timeSource.${row.timeSource}`) },
    { label: t("usagePerformance.sessionId"), value: row.sessionId },
    { label: t("usagePerformance.requestId"), value: row.requestId },
    { label: t("usagePerformance.errorType"), value: row.errorType },
    ...timings.filter(([, value]) => value !== null).map(([key, value]) => ({ label: t(key), value: duration(value, language) })),
    ...tokenValues.filter(([, value]) => value !== null).map(([key, value]) => ({ label: t(key), value: tokens(value) })),
    { label: t("usagePerformance.speedColumn"), value: row.outputTokensPerSecond === null ? null : speed(row.outputTokensPerSecond, language) },
    { label: t("usagePerformance.cost"), value: row.estimatedCostUsd === null ? null : cost(row.estimatedCostUsd, language) },
    { label: t("usagePerformance.attempts"), value: row.attemptCount === null ? null : number(row.attemptCount, language) },
  ];
  return <div className="asb-request-usage-details"><FactList rows={facts.filter((fact) => fact.value !== null)} /></div>;
}
