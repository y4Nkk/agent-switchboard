import { Cell, Pie, PieChart, ResponsiveContainer } from "recharts";
import { useCountUp } from "@/hooks/use-count-up";
import { TOKEN_UNIT, formatCompactTokenCount, formatTokenValue } from "../../lib/token-format";
import { formatUsageValue } from "../../lib/usage-format";
import type { ModelUsageDistributionItem } from "./chart-data";

const TOP_MODEL_COUNT = 5;
const SERIES_TONE_COUNT = 5;

type DisplayItem = ModelUsageDistributionItem & {
  otherCount: number | null;
};

interface Props {
  items: ModelUsageDistributionItem[];
  ariaLabel: string;
  emptyMessage: string;
}

/** A model ledger rendered as one prominent ring with an adjacent, auditable
 * legend. Only the tail is combined, and its label records that fact. */
export function ModelUsageDistributionChart({ items, ariaLabel, emptyMessage }: Props) {
  const visible = resolveDisplayItems(items);
  if (visible.length === 0) {
    return <p className="asb-chart-empty" role="status">{emptyMessage || "暂无可用模型构成数据。"}</p>;
  }

  return <DonutCard visible={visible} ariaLabel={ariaLabel} />;
}

function DonutCard({ visible, ariaLabel }: { visible: DisplayItem[]; ariaLabel: string }) {
  const total = visible.reduce((sum, item) => sum + item.value, 0);
  const display = useCountUp(Math.round(total));
  return (
    <figure
      className="bui-scope flex w-full min-w-0 flex-col gap-6 rounded-3xl bg-background-secondary-default p-6"
      aria-label={ariaLabel}
    >
      <figcaption className="text-title-3-semibold text-text-primary">模型构成</figcaption>
      <div className="flex min-w-0 flex-col items-center gap-8 lg:flex-row lg:items-center lg:gap-12">
        <div className="relative h-72 w-72 shrink-0">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie
                data={visible}
                dataKey="value"
                nameKey="label"
                innerRadius="61%"
                outerRadius="90%"
                paddingAngle={1}
                stroke="var(--color-background-secondary-default)"
                strokeWidth={4}
                isAnimationActive={false}
              >
                {visible.map((item, index) => (
                  <Cell
                    key={item.id}
                    fill={item.otherCount === null ? seriesColor(index) : "var(--color-chart-neutral)"}
                  />
                ))}
              </Pie>
            </PieChart>
          </ResponsiveContainer>
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
            <span className="animate-number-fade text-title-1-medium text-text-primary tabular-nums">
              {formatCompactTokenCount(display)}
            </span>
            <span className="text-body-medium text-text-tertiary">{TOKEN_UNIT}</span>
          </div>
        </div>
        <ol className="flex w-full min-w-0 flex-col divide-y divide-separator-border">
          {visible.map((item, index) => {
            const percent = (item.value / total) * 100;
            const label = item.otherCount === null ? item.label : `其他（${item.otherCount} 个模型）`;
            const color = item.otherCount === null ? seriesColor(index) : "var(--color-chart-neutral)";
            return (
              <li key={item.id} className="min-w-0 py-3 first:pt-0 last:pb-0">
                <div className="flex w-full min-w-0 items-center gap-2">
                  <span className="size-2.5 shrink-0 rounded-full" style={{ backgroundColor: color }} aria-hidden />
                  <span className="min-w-0 flex-1 truncate text-body-medium text-text-primary" title={label}>
                    {label}
                  </span>
                  <span className="shrink-0 text-body-medium text-text-tertiary tabular-nums">
                    {formatUsageValue(percent, "%")}
                  </span>
                </div>
                <p className="mt-1 ml-5 text-body-medium text-text-secondary tabular-nums">
                  {formatTokenValue(item.value)}
                </p>
              </li>
            );
          })}
        </ol>
      </div>
    </figure>
  );
}

/** Series hues ride the BoardUI chart tokens, which this app re-tints to its
 * own Frosted Relay series colors in boardui.css. */
function seriesColor(index: number): string {
  return `var(--color-chart-${(index % SERIES_TONE_COUNT) + 1})`;
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
