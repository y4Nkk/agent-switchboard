import type { CSSProperties } from "react";
import { chartTone, formatChartValue, type ModelUsageDistributionItem } from "./chart-data";

const TOP_MODEL_COUNT = 5;

type DistributionStyle = CSSProperties & {
  "--asb-chart-distribution-width": string;
};

interface DisplayItem extends ModelUsageDistributionItem {
  otherCount: number | null;
}

interface Props {
  items: ModelUsageDistributionItem[];
  ariaLabel: string;
  emptyMessage: string;
}

/** A compact top-model composition. Only the tail is combined, and its label
 * explicitly records how many real models it represents. */
export function ModelUsageDistributionChart({ items, ariaLabel, emptyMessage }: Props) {
  const visible = resolveDisplayItems(items);
  if (visible.length === 0) {
    return <p className="asb-chart-empty" role="status">{emptyMessage || "暂无可用模型构成数据。"}</p>;
  }

  const total = visible.reduce((sum, item) => sum + item.value, 0);
  return (
    <section className="asb-chart-distribution" aria-label={ariaLabel}>
      <ol className="asb-chart-distribution-list">
        {visible.map((item, index) => {
          const percent = (item.value / total) * 100;
          const style: DistributionStyle = { "--asb-chart-distribution-width": `${percent}%` };
          const label = item.otherCount === null ? item.label : `其他（${item.otherCount} 个模型）`;
          return (
            <li key={item.id} className="asb-chart-distribution-item" data-tone={chartTone(index)}>
              <div className="asb-chart-distribution-head">
                <span className="asb-chart-distribution-label" title={label}>{label}</span>
                <span className="asb-chart-distribution-value">{formatChartValue(item.value)}</span>
              </div>
              <div
                className="asb-chart-distribution-track"
                role="progressbar"
                aria-label={`${label} 占模型消耗总计的比例`}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(percent)}
                aria-valuetext={`${formatChartValue(percent)}%，${formatChartValue(item.value)}`}
              >
                <span className="asb-chart-distribution-fill" style={style} />
              </div>
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function resolveDisplayItems(items: ModelUsageDistributionItem[]): DisplayItem[] {
  const realItems = items
    .filter((item) => Number.isFinite(item.value) && item.value > 0)
    .map((item, index) => ({
      ...item,
      id: item.id.trim() || `model-${index}`,
      label: item.label.trim() || "未记录模型",
      otherCount: null,
    }))
    .sort((left, right) => right.value - left.value);
  const leading = realItems.slice(0, TOP_MODEL_COUNT);
  const remainder = realItems.slice(TOP_MODEL_COUNT);
  if (remainder.length === 0) return leading;

  return [
    ...leading,
    {
      id: "model-usage-other",
      label: "其他",
      value: remainder.reduce((sum, item) => sum + item.value, 0),
      otherCount: remainder.length,
    },
  ];
}
