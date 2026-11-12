import type { RequestUsageMetrics } from "../../api/client";
import { useI18n } from "../../i18n";
import { RequestUsageBar } from "./RequestUsageBar";
import { number, percent } from "./format";

export function RequestUsageQuality({ metrics }: { metrics: RequestUsageMetrics }) {
  const { t } = useI18n();
  return <section className="asb-request-usage-breakdown-block asb-request-usage-quality">
    <h3 className="asb-group-title">{t("usagePerformance.quality")}</h3>
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
    <figcaption className="asb-request-usage-breakdown-row">
      <span>{t("usagePerformance.failures")}</span><strong>{number(m.failed, language)}</strong>
    </figcaption>
    {m.failed > 0 ? <>
      <div className="asb-request-failure-stack" aria-hidden="true">
        {failures.filter((item) => item.value > 0).map((item) => <span key={item.id}
          data-failure={item.id} style={{ width: `${item.value / m.failed * 100}%` }} />)}
      </div>
      <ul className="asb-request-failure-legend">
        {failures.map((item) => <li key={item.id}>
          <span className="asb-request-failure-dot" data-failure={item.id} aria-hidden="true" />
          <span>{t(item.key)}</span><strong>{number(item.value, language)}</strong>
        </li>)}
      </ul>
    </> : <p className="asb-request-usage-caption">{t("usagePerformance.noFailures")}</p>}
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
    <div className="asb-request-usage-breakdown-row"><span>{label}</span><strong>{percent(ratio, language)}</strong></div>
    <RequestUsageBar value={ratio} maximum={100} />
    <span className="asb-request-usage-caption">{number(value, language)} / {number(total, language)} · {denominator}</span>
  </div>;
}
