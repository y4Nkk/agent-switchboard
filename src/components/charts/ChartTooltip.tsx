import { formatChartTimestamp, formatChartValue } from "./chart-data";

interface Props {
  point: {
    label: string;
    value: number;
    unit: string | null;
    timestamp: number;
  } | null;
}

/** The selected data point is rendered as text below the plot instead of an
 * overlapping floating card, keeping dense graph reads and keyboard focus clear. */
export function ChartTooltip({ point }: Props) {
  if (point === null) return null;

  return (
    <p className="asb-chart-tooltip" role="status" aria-live="polite">
      <span>{point.label}</span>
      <span>{formatChartValue(point.value, point.unit)}</span>
      <time dateTime={new Date(point.timestamp).toISOString()}>{formatChartTimestamp(point.timestamp)}</time>
    </p>
  );
}
