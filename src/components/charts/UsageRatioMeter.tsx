import type { CSSProperties } from "react";
import { formatChartValue } from "./chart-data";

type MeterStyle = CSSProperties & {
  "--asb-usage-ratio-meter-width": string;
};

interface Props {
  percent: number | null;
  ariaLabel: string;
}

/** The table-scale ratio meter keeps its number alongside the rail so a color
 * fill never has to carry the percentage by itself. */
export function UsageRatioMeter({ percent, ariaLabel }: Props) {
  if (percent === null || !Number.isFinite(percent)) {
    return <span className="asb-usage-ratio-meter asb-usage-ratio-meter-empty" aria-label={ariaLabel}>—</span>;
  }

  const clamped = Math.min(100, Math.max(0, percent));
  const style: MeterStyle = { "--asb-usage-ratio-meter-width": `${clamped}%` };
  return (
    <span
      className="asb-usage-ratio-meter"
      role="progressbar"
      aria-label={ariaLabel}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamped)}
      aria-valuetext={formatChartValue(clamped) + "%"}
    >
      <span className="asb-usage-ratio-meter-track" aria-hidden="true">
        <span className="asb-usage-ratio-meter-fill" style={style} />
      </span>
      <span className="asb-usage-ratio-meter-value">{formatChartValue(clamped)}%</span>
    </span>
  );
}
