import type { RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { CompositionRing } from "../charts/CompositionRing";
import { RequestUsageRatioRing } from "./RequestUsageMicrocharts";
import { number, percent } from "./format";

const FAILURE_COLORS = {
  limited: "var(--asb-warning)",
  server: "var(--asb-danger)",
  unknown: "var(--asb-text-muted)",
  other: "var(--asb-claude)",
} as const;

export function RequestUsageQuality({ metrics }: { metrics: RequestUsageMetrics }) {
  const { t } = useI18n();
  return <section className="asb-request-usage-breakdown-block asb-request-usage-quality">
    <h4 className="asb-group-title">{t("usagePerformance.quality")}</h4>
    <div className="asb-request-quality-grid">
      <FailureComposition metrics={metrics} />
      <RetryOverview metrics={metrics} />
    </div>
  </section>;
}

function FailureComposition({ metrics: m }: { metrics: RequestUsageMetrics }) {
  const { t, language } = useI18n();
  const failures = [
    { id: "limited", key: "usagePerformance.rateLimited", value: m.rateLimited },
    { id: "server", key: "usagePerformance.serverErrors", value: m.upstreamErrors },
    { id: "unknown", key: "usagePerformance.unclassified", value: m.unclassifiedFailures },
    { id: "other", key: "usagePerformance.otherFailures", value: m.failed - m.rateLimited - m.upstreamErrors - m.unclassifiedFailures },
  ] as const;
  return <figure className="asb-request-quality-chart">
    <figcaption className="asb-request-usage-breakdown-row"><span>{t("usagePerformance.failures")}</span></figcaption>
    {m.failed > 0 ? <div className="asb-request-failure-composition">
      <div className="asb-request-failure-ring">
        <CompositionRing parts={failures.map(({ id, value }) => ({ id, value, color: FAILURE_COLORS[id] }))} />
        <strong>{number(m.failed, language)}</strong>
      </div>
      <ul className="asb-request-failure-legend">
        {failures.map((item) => <li key={item.id}>
          <span className="asb-request-failure-dot" style={{ backgroundColor: FAILURE_COLORS[item.id] }} aria-hidden="true" />
          <span>{t(item.key)}</span><strong>{number(item.value, language)} · {percent(item.value / m.failed * 100, language)}</strong>
        </li>)}
      </ul>
    </div> : <p className="asb-request-usage-caption">{t("usagePerformance.noFailures")}</p>}
  </figure>;
}

function RetryOverview({ metrics: m }: { metrics: RequestUsageMetrics }) {
  const { t, language } = useI18n();
  return <div className="asb-request-quality-chart">
    <div className="asb-request-usage-breakdown-row">
      <span>{t("usagePerformance.retries")}</span>
      <strong>{m.attemptSamples > 0 ? t("usagePerformance.retryTotal", { count: number(m.retries, language) }) : "—"}</strong>
    </div>
    {m.attemptSamples > 0 ? <>
      <RetryRatio label={t("usagePerformance.retryRecords")} value={m.retryRecords} total={m.attemptSamples}
        denominator={t("usagePerformance.attemptCoverage")} />
      {m.retryRecords > 0 ? <RetryRatio label={t("usagePerformance.recovered")} value={m.recoveredRecords} total={m.retryRecords}
        denominator={t("usagePerformance.retryRecords")} />
        : <p className="asb-request-usage-caption">{t("usagePerformance.noRetries")}</p>}
    </> : <p className="asb-request-usage-caption">{t("usagePerformance.retryUnavailable")}</p>}
  </div>;
}

function RetryRatio({ label, value, total, denominator }: {
  label: string; value: number; total: number; denominator: string;
}) {
  const { language } = useI18n();
  const ratio = total > 0 ? value / total * 100 : null;
  return <div className="asb-request-retry-ratio">
    <RequestUsageRatioRing value={ratio} />
    <div className="asb-request-retry-readout">
      <div className="asb-request-usage-breakdown-row"><span>{label}</span><strong>{percent(ratio, language)}</strong></div>
      <span className="asb-request-usage-caption">{number(value, language)} / {number(total, language)} · {denominator}</span>
    </div>
  </div>;
}
