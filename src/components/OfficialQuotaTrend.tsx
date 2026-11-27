import { useState } from "react";
import { officialQuotaWindowName } from "../lib/usage-format";
import type { UsageHistorySeries } from "../api/client";
import { useI18n } from "../i18n";
import { UsageTrendChart } from "./charts/UsageTrendChart";
import { CollapsibleSection } from "./Collapsible";
import { Time } from "./Time";

interface Props {
  series: UsageHistorySeries[];
  loading: boolean;
  error: string | null;
  ariaLabel: string;
}

/** One account-level official quota trend. Only normalized successful reads
 * enter this series; a stale or failed response never adds a point. */
export function OfficialQuotaTrend({ series, loading, error, ariaLabel }: Props) {
  const { t } = useI18n();
  const [samplingOpen, setSamplingOpen] = useState(false);
  const quotaSeries = series.filter((entry) => entry.metric === "usedPercent" && entry.points.length > 0);
  const sampled = quotaSeries.filter((entry) => entry.totalPoints > entry.points.length);

  return (
    <section className="asb-usage-history" aria-label={ariaLabel}>
      <h4 className="asb-section-title">{t("clientConfig.quota.trendTitle")}</h4>
      {loading && quotaSeries.length === 0 ? (
        <span className="asb-skeleton asb-usage-history-skeleton" role="status" aria-label={t("clientConfig.quota.historyLoading")} />
      ) : (
        <UsageTrendChart
          size="compact"
          series={quotaSeries.map(({ id, label, unit, points }) => ({ id, label: officialQuotaWindowName(label), unit: unit ?? "%", points }))}
          ariaLabel={ariaLabel}
          emptyMessage={t("clientConfig.quota.trendEmpty")}
          valueKind="percentage"
        />
      )}
      {sampled.length > 0 && <CollapsibleSection quiet open={samplingOpen} onOpenChange={setSamplingOpen}
        label={t("usage.history.samplingDetails")}>
        {sampled.map((entry) => <p key={entry.id} className="asb-scope-note">
          {officialQuotaWindowName(entry.label)} · {t("usage.history.sampled", { shown: entry.points.length, total: entry.totalPoints })}
          {" · "}<Time iso={entry.firstAt} />{" — "}<Time iso={entry.lastAt} />
        </p>)}
      </CollapsibleSection>}
      {error && <p className="asb-warn-text" role="alert">{t("clientConfig.quota.historyError", { error })}</p>}
    </section>
  );
}
