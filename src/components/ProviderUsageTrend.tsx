import type { UsageHistoryMetric, UsageHistorySeries } from "../api/client";
import { UsageTrendChart } from "./charts/UsageTrendChart";
import type { UsageTrendSeries } from "./charts/chart-data";

interface Props {
  providerName: string;
  series: UsageHistorySeries[];
  loading: boolean;
  error: string | null;
}

interface TrendGroup {
  unit: string | null;
  series: UsageTrendSeries[];
}

interface MetricSelection {
  metric: UsageHistoryMetric;
  heading: string;
  groups: TrendGroup[];
}

/** Renders every comparable provider metric. The precise current readings
 * stay in UsageReadingsTable; this view is for changes across real reads. */
export function ProviderUsageTrend({ providerName, series, loading, error }: Props) {
  const selections = selectProviderMetrics(series);
  const hasSeries = selections.length > 0;
  const heading = selections.length === 1 ? selections[0].heading : "用量趋势";

  return (
    <section className="asb-usage-history" aria-label={`${providerName}${heading}`}>
      <h4>{heading}</h4>
      {loading && !hasSeries ? (
        <p className="asb-provider-usage-state" role="status">正在读取历史记录…</p>
      ) : hasSeries ? (
        selections.map((selection) => (
          <div key={selection.metric} className="asb-usage-history-metric">
            {selections.length > 1 && <h5>{selection.heading}</h5>}
            {selection.groups.map((group) => (
              <div key={group.unit ?? "unitless"} className="asb-usage-history-group">
                {selection.groups.length > 1 && <p className="asb-usage-history-unit">{group.unit ?? "未标注单位"}</p>}
                <UsageTrendChart
                  size="compact"
                  series={group.series}
                  ariaLabel={`${providerName}${selection.heading}${group.unit ? `（${group.unit}）` : ""}`}
                  emptyMessage="尚无可比较的历史读数。"
                  sumAcrossSeries
                />
              </div>
            ))}
          </div>
        ))
      ) : (
        <UsageTrendChart
          size="compact"
          series={[]}
          ariaLabel={`${providerName}${heading}`}
          emptyMessage="成功读取后会在这里显示趋势。"
        />
      )}
      {error && <p className="asb-warn-text" role="alert">历史记录不可用：{error}</p>}
    </section>
  );
}

function selectProviderMetrics(series: UsageHistorySeries[]): MetricSelection[] {
  return [
    { metric: "remaining" as UsageHistoryMetric, heading: "余额趋势" },
    { metric: "used" as UsageHistoryMetric, heading: "已用趋势" },
  ].flatMap((selection) => {
    const groups = groupByUnit(series.filter((entry) => entry.metric === selection.metric && entry.points.length > 0));
    return groups.length ? [{ ...selection, groups }] : [];
  });
}

function groupByUnit(series: UsageHistorySeries[]): TrendGroup[] {
  const groups = new Map<string, TrendGroup>();
  for (const entry of series) {
    const unit = entry.unit?.trim() || null;
    const key = unit ?? "";
    const group = groups.get(key) ?? { unit, series: [] };
    group.series.push({
      id: entry.id,
      label: entry.label,
      unit,
      points: entry.points,
    });
    groups.set(key, group);
  }
  return [...groups.values()];
}
