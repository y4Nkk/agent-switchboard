import { Cell, Pie, PieChart } from "recharts";
import { useCountUp } from "@/hooks/use-count-up";
import { useI18n } from "../../i18n";
import { TOKEN_UNIT } from "../../lib/token-format";
import { formatCompactUsageValue, formatUsageValue } from "../../lib/usage-format";
import type { ModelUsageSlice } from "../model-usage-display";
import { Tooltip } from "../Tooltip";
import { ChartFrame } from "./ChartFrame";
import { chartSeriesColor } from "./chart-data";

interface Props {
  items: ModelUsageSlice[];
  ariaLabel: string;
}

export function ModelUsageDistributionChart({ items, ariaLabel }: Props) {
  const { t } = useI18n();
  const total = items.reduce((sum, item) => sum + item.value, 0);
  const display = useCountUp(Math.round(total));
  return (
    <figure className="asb-usage-donut-card" aria-label={ariaLabel}>
      <figcaption className="asb-usage-donut-title">{t("usage.composition.title")}</figcaption>
      <div className="asb-usage-donut-layout">
        <div className="asb-usage-donut-stage">
          <ChartFrame>
            <PieChart>
              <Pie
                data={items}
                dataKey="value"
                nameKey="label"
                innerRadius="64%"
                outerRadius="98%"
                stroke="var(--asb-usage-surface)"
                strokeWidth={2}
                isAnimationActive={false}
              >
                {items.map((item, index) => (
                  <Cell key={item.id} fill={chartSeriesColor(index, "local-token")} />
                ))}
              </Pie>
            </PieChart>
          </ChartFrame>
          <div className="asb-usage-donut-center">
            <span className="asb-usage-donut-center-value">
              {formatCompactUsageValue(display)}
            </span>
            <span className="asb-usage-donut-center-unit">{TOKEN_UNIT}</span>
          </div>
        </div>
        <ol className="asb-usage-donut-legend">
          {items.map((item, index) => {
            const percent = (item.value / total) * 100;
            return (
              <li key={item.id} className="asb-usage-donut-row">
                <div className="asb-usage-donut-row-head">
                  <span className="asb-usage-donut-swatch" style={{ backgroundColor: chartSeriesColor(index, "local-token") }} aria-hidden />
                  <Tooltip asChild label={item.label}><span className="asb-usage-donut-name">{item.label}</span></Tooltip>
                  <span className="asb-usage-donut-percent">{formatUsageValue(percent, "%")}</span>
                </div>
                <p className="asb-usage-donut-value">{formatCompactUsageValue(item.value)} {TOKEN_UNIT}</p>
              </li>
            );
          })}
        </ol>
      </div>
    </figure>
  );
}
