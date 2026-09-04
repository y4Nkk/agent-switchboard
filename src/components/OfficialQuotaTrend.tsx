import type { UsageHistorySeries } from "../api/client";
import { UsageTrendChart } from "./charts/UsageTrendChart";

interface Props {
  series: UsageHistorySeries[];
  loading: boolean;
  error: string | null;
  ariaLabel: string;
}

/** One account-level official quota trend. Only normalized successful reads
 * enter this series; a stale or failed response never adds a point. */
export function OfficialQuotaTrend({ series, loading, error, ariaLabel }: Props) {
  const quotaSeries = series
    .filter((entry) => entry.metric === "usedPercent" && entry.points.length > 0)
    .map((entry) => ({
      id: entry.id,
      label: entry.label,
      unit: entry.unit ?? "%",
      points: entry.points,
    }));

  return (
    <section className="asb-usage-history" aria-label={ariaLabel}>
      <h4>额度趋势</h4>
      {loading && quotaSeries.length === 0 ? (
        <p className="asb-provider-usage-state" role="status">正在读取历史记录…</p>
      ) : (
        <UsageTrendChart
          size="compact"
          series={quotaSeries}
          ariaLabel={ariaLabel}
          emptyMessage="成功读取官方额度后会在这里显示趋势。"
          valueKind="percentage"
        />
      )}
      {error && <p className="asb-warn-text" role="alert">历史记录不可用：{error}</p>}
    </section>
  );
}
