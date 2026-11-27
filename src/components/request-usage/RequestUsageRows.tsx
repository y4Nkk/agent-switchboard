import { Activity, Clock3, Gauge, Hash, MessagesSquare, Tag, Timer, TriangleAlert } from "lucide-react";
import { useId, useState } from "react";
import type { RequestUsageReport, RequestUsageRow } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Collapsible, CollapsibleChevron } from "../Collapsible";
import { FactList, type FactListRow } from "../FactList";
import { Pagination } from "../Pagination";
import { Time } from "../Time";
import { RECORD_METRICS, type RequestUsageRecordMetric } from "./chart-metrics";
import { RequestUsagePointAxis, RequestUsagePoints } from "./RequestUsagePoints";
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
      <RequestUsagePointAxis end={maximum > 0 ? definition.format(maximum, language) : null} />
      <ul className="asb-request-usage-observations">
        {report.rows.map((row) => {
          const open = expanded === row.id;
          const detailsId = `${id}-${row.id}`;
          return <li key={row.id}>
            <button type="button" className="asb-request-usage-observation asb-request-usage-record"
              data-outcome={row.outcome} aria-expanded={open} aria-controls={open ? detailsId : undefined}
              onClick={() => setExpanded(open ? null : row.id)}>
              <span className="asb-request-usage-observation-label">
                <span className="asb-code">{row.model ?? t("usage.model.unrecorded")}</span>
                <span className="asb-request-usage-caption">
                  <Time iso={new Date(row.atMs).toISOString()} precision="second" /> · {clientName(row.app)}
                </span>
              </span>
              <RequestUsagePoints value={row[metric]} maximum={maximum} />
              <span className="asb-request-usage-observation-value">
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
  const facts: FactListRow[] = [
    { label: t("usagePerformance.eventName"), icon: Activity, value: row.eventName },
    { label: t("usagePerformance.eventKind"), icon: Tag, value: row.eventKind },
    { label: t("usagePerformance.timeSource"), icon: Clock3, value: t(`usagePerformance.timeSource.${row.timeSource}`) },
    { label: t("usagePerformance.sessionId"), icon: Hash, format: "code", value: row.sessionId },
    { label: t("usagePerformance.requestId"), icon: Hash, format: "code", value: row.requestId },
    { label: t("usagePerformance.errorType"), icon: TriangleAlert, tone: "warning", value: row.errorType },
    ...timings.filter(([, value]) => value !== null).map<FactListRow>(([key, value]) => ({
      label: t(key), icon: Timer, format: "number", value: duration(value, language),
    })),
    ...tokenValues.filter(([, value]) => value !== null).map<FactListRow>(([key, value]) => ({
      label: t(key), icon: MessagesSquare, format: "number", value: tokens(value),
    })),
    { label: t("usagePerformance.speedColumn"), icon: Gauge, format: "number",
      value: row.outputTokensPerSecond === null ? null : speed(row.outputTokensPerSecond, language) },
    { label: t("usagePerformance.cost"), icon: Tag, format: "number",
      value: row.estimatedCostUsd === null ? null : cost(row.estimatedCostUsd, language) },
    { label: t("usagePerformance.attempts"), icon: Activity, format: "number",
      value: row.attemptCount === null ? null : number(row.attemptCount, language) },
  ];
  return <div className="asb-request-usage-details"><FactList rows={facts.filter((fact) => fact.value !== null)} /></div>;
}
