import { useMemo, useState } from "react";
import { ChartLegend } from "./ChartLegend";
import { ChartTooltip } from "./ChartTooltip";
import {
  chartTone,
  formatChartAxisTime,
  formatChartTimestamp,
  formatChartValue,
  prepareTrendSeries,
  trendSeriesShareUnit,
  type PreparedTrendPoint,
  type PreparedTrendSeries,
  type UsageTrendSeries,
} from "./chart-data";

const VIEWBOX_WIDTH = 720;
const VIEWBOX_HEIGHT = 256;
const PLOT_LEFT = 66;
const PLOT_RIGHT = 20;
const PLOT_TOP = 16;
const PLOT_BOTTOM = 42;
const Y_TICK_COUNT = 3;

interface ActivePoint {
  label: string;
  value: number;
  unit: string | null;
  timestamp: number;
}

interface Props {
  series: UsageTrendSeries[];
  ariaLabel: string;
  emptyMessage: string;
}

/** A single-unit, actual-point trend plot. It preserves input point order and
 * never fills missing timestamps with inferred values. */
export function UsageTrendChart({ series, ariaLabel, emptyMessage }: Props) {
  const prepared = useMemo(() => prepareTrendSeries(series), [series]);
  const [activePoint, setActivePoint] = useState<ActivePoint | null>(null);

  if (prepared.length === 0) {
    return <p className="asb-chart-empty" role="status">{emptyMessage || "暂无可用趋势数据。"}</p>;
  }

  if (!trendSeriesShareUnit(prepared)) {
    return <p className="asb-chart-empty" role="alert">无法将不同单位的数据放在同一趋势图中。</p>;
  }

  const geometry = resolveGeometry(prepared);
  const unit = prepared[0]?.unit ?? null;
  const setActive = (entry: PreparedTrendSeries, point: PreparedTrendPoint) => {
    setActivePoint({ label: entry.label, value: point.value, unit: entry.unit, timestamp: point.timestamp });
  };

  return (
    <section className="asb-chart" aria-label={ariaLabel}>
      <div className="asb-chart-plot">
        <svg className="asb-chart-svg" viewBox={`0 0 ${VIEWBOX_WIDTH} ${VIEWBOX_HEIGHT}`} role="group" aria-label={ariaLabel}>
          <g aria-hidden="true">
            {geometry.yTicks.map((tick) => (
              <g key={tick.value}>
                <line
                  className="asb-chart-gridline"
                  x1={PLOT_LEFT}
                  x2={VIEWBOX_WIDTH - PLOT_RIGHT}
                  y1={tick.y}
                  y2={tick.y}
                />
                <text className="asb-chart-axis-label" x={PLOT_LEFT - 8} y={tick.y} textAnchor="end" dominantBaseline="middle">
                  {formatChartValue(tick.value, unit)}
                </text>
              </g>
            ))}
            <line
              className="asb-chart-axis-line"
              x1={PLOT_LEFT}
              x2={VIEWBOX_WIDTH - PLOT_RIGHT}
              y1={VIEWBOX_HEIGHT - PLOT_BOTTOM}
              y2={VIEWBOX_HEIGHT - PLOT_BOTTOM}
            />
            {geometry.xLabels.map((tick) => (
              <text
                key={`${tick.x}-${tick.timestamp}`}
                className="asb-chart-axis-label"
                x={tick.x}
                y={VIEWBOX_HEIGHT - 16}
                textAnchor={tick.anchor}
              >
                {formatChartAxisTime(tick.timestamp)}
              </text>
            ))}
          </g>
          {prepared.map((entry, seriesIndex) => (
            <g key={entry.id} className="asb-chart-series" data-tone={chartTone(seriesIndex)}>
              <path className="asb-chart-line" d={trendPath(entry, geometry)} aria-hidden="true" />
              {entry.points.map((point) => {
                const pointLabel = `${entry.label}，${formatChartValue(point.value, entry.unit)}，${formatChartTimestamp(point.timestamp)}`;
                return (
                  <circle
                    key={`${entry.id}-${point.index}`}
                    className="asb-chart-point"
                    cx={xFor(point.timestamp, geometry)}
                    cy={yFor(point.value, geometry)}
                    tabIndex={0}
                    role="img"
                    aria-label={pointLabel}
                    onMouseEnter={() => setActive(entry, point)}
                    onMouseLeave={() => setActivePoint(null)}
                    onFocus={() => setActive(entry, point)}
                    onBlur={() => setActivePoint(null)}
                  >
                    <title>{pointLabel}</title>
                  </circle>
                );
              })}
            </g>
          ))}
        </svg>
      </div>
      <ChartTooltip point={activePoint} />
      <ChartLegend items={prepared.map((entry) => ({ id: entry.id, label: entry.label }))} />
    </section>
  );
}

interface Geometry {
  minTimestamp: number;
  maxTimestamp: number;
  minValue: number;
  maxValue: number;
  yTicks: Array<{ value: number; y: number }>;
  xLabels: Array<{ timestamp: number; x: number; anchor: "start" | "middle" | "end" }>;
}

function resolveGeometry(series: PreparedTrendSeries[]): Geometry {
  const points = series.flatMap((entry) => entry.points);
  const timestamps = points.map((point) => point.timestamp);
  const minTimestamp = Math.min(...timestamps);
  const maxTimestamp = Math.max(...timestamps);
  const observedMinimum = Math.min(...points.map((point) => point.value));
  const observedMaximum = Math.max(...points.map((point) => point.value));
  const minValue = Math.min(0, observedMinimum);
  const maximumWithBaseline = Math.max(0, observedMaximum);
  const maxValue = maximumWithBaseline === minValue ? minValue + 1 : maximumWithBaseline;
  const plotHeight = VIEWBOX_HEIGHT - PLOT_TOP - PLOT_BOTTOM;
  const yTicks = Array.from({ length: Y_TICK_COUNT }, (_, index) => {
    const value = maxValue - ((maxValue - minValue) * index) / (Y_TICK_COUNT - 1);
    return { value, y: PLOT_TOP + (plotHeight * index) / (Y_TICK_COUNT - 1) };
  });

  const xLabels = minTimestamp === maxTimestamp
    ? [{ timestamp: minTimestamp, x: (PLOT_LEFT + VIEWBOX_WIDTH - PLOT_RIGHT) / 2, anchor: "middle" as const }]
    : [
        { timestamp: minTimestamp, x: PLOT_LEFT, anchor: "start" as const },
        { timestamp: maxTimestamp, x: VIEWBOX_WIDTH - PLOT_RIGHT, anchor: "end" as const },
      ];

  return { minTimestamp, maxTimestamp, minValue, maxValue, yTicks, xLabels };
}

function xFor(timestamp: number, geometry: Geometry): number {
  const plotWidth = VIEWBOX_WIDTH - PLOT_LEFT - PLOT_RIGHT;
  if (geometry.minTimestamp === geometry.maxTimestamp) return PLOT_LEFT + plotWidth / 2;
  return PLOT_LEFT + ((timestamp - geometry.minTimestamp) / (geometry.maxTimestamp - geometry.minTimestamp)) * plotWidth;
}

function yFor(value: number, geometry: Geometry): number {
  const plotHeight = VIEWBOX_HEIGHT - PLOT_TOP - PLOT_BOTTOM;
  return PLOT_TOP + (1 - (value - geometry.minValue) / (geometry.maxValue - geometry.minValue)) * plotHeight;
}

function trendPath(entry: PreparedTrendSeries, geometry: Geometry): string {
  return entry.points
    .map((point, index) => `${index === 0 ? "M" : "L"}${xFor(point.timestamp, geometry)} ${yFor(point.value, geometry)}`)
    .join(" ");
}
