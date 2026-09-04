import { useId, useMemo, useState } from "react";
import { Area, ComposedChart, Line, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { useCountUp } from "@/hooks/use-count-up";
import { cx } from "@/utils/cx";
import {
  formatChartAxisTime,
  formatChartAxisValue,
  formatChartTimestamp,
  formatChartValue,
  prepareTrendSeries,
  trendSeriesShareUnit,
  type PreparedTrendSeries,
  type UsageTrendSeries,
} from "./chart-data";

interface Props {
  series: UsageTrendSeries[];
  ariaLabel: string;
  emptyMessage: string;
  /** Visible heading for the standalone local-usage analysis surface. */
  title?: string;
  /** The headline aggregates every series at one timestamp. Only set this
   * when the shared unit makes the sum meaningful; otherwise the headline
   * tracks the first series (e.g. one quota window's percentage). */
  sumAcrossSeries?: boolean;
  /** "default" is the full showcase card for the usage page; "compact" fits
   * trend charts embedded in provider and official-quota panels. */
  size?: "default" | "compact";
}

const SERIES_TONE_COUNT = 5;
const AXIS_TICK = { fontSize: 12, fill: "var(--color-text-tertiary)" };
/** The BoardUI chart card, fed by real recorded points only. It preserves
 * input point order and never fills missing timestamps with inferred values;
 * series that were read at different times simply leave gaps instead of
 * inventing points. */
export function UsageTrendChart({
  series,
  ariaLabel,
  emptyMessage,
  title,
  sumAcrossSeries = false,
  size = "default",
}: Props) {
  const prepared = useMemo(() => prepareTrendSeries(series), [series]);

  if (prepared.length === 0) {
    return <p className="asb-chart-empty" role="status">{emptyMessage || "暂无可用趋势数据。"}</p>;
  }

  if (!trendSeriesShareUnit(prepared)) {
    return <p className="asb-chart-empty" role="alert">无法将不同单位的数据放在同一趋势图中。</p>;
  }

  return (
    <TrendCard prepared={prepared} sumAcrossSeries={sumAcrossSeries} ariaLabel={ariaLabel} title={title} size={size} />
  );
}

function TrendCard({
  prepared,
  sumAcrossSeries,
  ariaLabel,
  title,
  size,
}: {
  prepared: PreparedTrendSeries[];
  sumAcrossSeries: boolean;
  ariaLabel: string;
  title?: string;
  size: "default" | "compact";
}) {
  const gradientId = useId();
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const compact = size === "compact";
  const standalone = !compact && title !== undefined;

  const rows = mergeSeriesRows(prepared);
  const unit = prepared[0]?.unit ?? null;
  const pointCount = prepared.reduce((total, entry) => total + entry.points.length, 0);

  // Rest state headlines the newest real reading; hover headlines the hovered
  // timestamp. No aggregate ever spans time — only series at one timestamp.
  const activeRow = activeIndex !== null && activeIndex < rows.length ? rows[activeIndex] : null;
  const restRow = rows[rows.length - 1];
  const row = activeRow ?? restRow;
  const figure = rowFigure(row, prepared, sumAcrossSeries);
  const display = useCountUp(Math.round(figure));

  const label = activeRow
    ? formatChartAxisTime(row.timestamp)
    : sumAcrossSeries
      ? "合计"
      : prepared[0].label;
  const caption = activeRow
    ? formatChartTimestamp(row.timestamp)
    : unit
      ? `单位：${unit}`
      : `${pointCount} 个真实读数`;

  return (
    <figure
      className={cx(
        "bui-scope flex w-full min-w-0 flex-col bg-background-secondary-default",
        compact
          ? "h-56 gap-4 rounded-2xl px-4 pt-3.5 pb-2.5"
          : standalone
            ? "h-96 gap-6 rounded-3xl p-6"
            : "h-[344px] gap-6 rounded-2xl px-4 pt-4 pb-3",
      )}
      aria-label={ariaLabel}
    >
      {title && <figcaption className="text-title-3-semibold text-text-primary">{title}</figcaption>}
      {/* Header: label over the count-up figure and its caption; legend on the right */}
      <div
        className={cx(
          "flex w-full flex-col gap-3",
          standalone ? "lg:flex-row lg:items-start lg:justify-between" : "sm:flex-row sm:items-start sm:justify-between",
        )}
      >
        <div className="flex min-w-0 flex-col gap-0.5">
          <p className="w-full text-body-medium text-text-secondary">{label}</p>
          <div className="flex w-full items-center gap-2">
            <p
              key={activeIndex ?? "rest"}
              className={cx(
                "animate-number-fade whitespace-nowrap text-text-primary tabular-nums",
                compact ? "text-title-2-medium" : "text-title-1-medium",
              )}
            >
              {formatChartValue(display, unit)}
            </p>
          </div>
          <p className="text-body-2-medium text-text-tertiary tabular-nums">{caption}</p>
        </div>
        <dl
          className={cx(
            "flex shrink-0 items-center text-body-2-medium text-text-secondary",
            standalone ? "flex-wrap gap-x-5 gap-y-2" : "gap-4",
          )}
        >
          {prepared.map((entry, index) => (
            <div key={entry.id} className="flex items-center gap-1.5">
              <span
                className="size-2 rounded-full"
                style={{ backgroundColor: seriesColor(index) }}
                aria-hidden
              />
              <dt className="whitespace-nowrap">{entry.label}</dt>
            </div>
          ))}
        </dl>
      </div>

      {/* Chart */}
      <div className="min-h-0 w-full flex-1">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={rows}
            margin={{ top: 4, right: 6, bottom: 0, left: 0 }}
            onMouseMove={(nextState) => {
              const index = Number(nextState.activeTooltipIndex);
              if (nextState.isTooltipActive && Number.isFinite(index)) setActiveIndex(index);
            }}
            onMouseLeave={() => setActiveIndex(null)}
          >
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={seriesColor(0)} stopOpacity={0.35} />
                <stop offset="100%" stopColor={seriesColor(0)} stopOpacity={0} />
              </linearGradient>
            </defs>
            <YAxis
              width={44}
              domain={[0, yMax(prepared) * 1.1]}
              tickCount={4}
              tickFormatter={(value: number) => formatChartAxisValue(value, unit)}
              tickLine={false}
              axisLine={false}
              tick={AXIS_TICK}
            />
            <XAxis
              dataKey="timestamp"
              type="number"
              scale="time"
              domain={["dataMin", "dataMax"]}
              tickFormatter={(value: number) => formatChartAxisTime(Number(value))}
              tickLine={false}
              axisLine={false}
              tickMargin={12}
              tick={AXIS_TICK}
            />
            <Tooltip
              content={() => null}
              cursor={{ stroke: "var(--color-chart-cursor)", strokeWidth: 1, strokeDasharray: "4 4" }}
            />
            {prepared.length === 1 && (
              <Area
                type="monotone"
                dataKey={prepared[0].id}
                stroke="none"
                fill={`url(#${gradientId})`}
                dot={false}
                connectNulls
                isAnimationActive={false}
              />
            )}
            {prepared.map((entry, index) => (
              <Line
                key={entry.id}
                type="monotone"
                dataKey={entry.id}
                name={entry.label}
                stroke={seriesColor(index)}
                strokeWidth={2.5}
                // A lone real point draws no line; show it as a dot instead of
                // leaving the only recorded value invisible.
                dot={entry.points.length === 1 ? { r: 3 } : false}
                activeDot={<ActiveDot color={seriesColor(index)} />}
                connectNulls
                isAnimationActive={false}
              />
            ))}
          </ComposedChart>
        </ResponsiveContainer>
      </div>
    </figure>
  );
}

/** The BoardUI hover marker: a soft halo behind a solid dot ringed by the
 * card surface. Recharts clones this element with the active point's
 * coordinates. */
function ActiveDot({ color, cx, cy }: { color: string; cx?: number; cy?: number }) {
  if (cx === undefined || cy === undefined) return null;
  return (
    <g>
      <circle cx={cx} cy={cy} r={7} fill={color} opacity={0.25} />
      <circle
        cx={cx}
        cy={cy}
        r={4}
        fill={color}
        stroke="var(--color-background-secondary-default)"
        strokeWidth={2}
      />
    </g>
  );
}

/** Series hues ride the BoardUI chart tokens, which this app re-tints to its
 * own Frosted Relay series colors in boardui.css. */
function seriesColor(index: number): string {
  return `var(--color-chart-${(index % SERIES_TONE_COUNT) + 1})`;
}

function yMax(series: PreparedTrendSeries[]): number {
  const max = Math.max(...series.flatMap((entry) => entry.points.map((point) => point.value)));
  return Math.max(max, 1);
}

function rowFigure(row: TrendRow, series: PreparedTrendSeries[], sumAcrossSeries: boolean): number {
  if (sumAcrossSeries) {
    return series.reduce((sum, entry) => {
      const value = row[entry.id];
      return typeof value === "number" ? sum + value : sum;
    }, 0);
  }
  return typeof row[series[0].id] === "number" ? (row[series[0].id] as number) : 0;
}

interface TrendRow {
  timestamp: number;
  [seriesId: string]: number | undefined;
}

/** Union of every real timestamp; each series contributes only the points it
 * actually recorded, so nothing is interpolated into the gaps. */
function mergeSeriesRows(series: PreparedTrendSeries[]): TrendRow[] {
  const timestamps = new Set<number>();
  for (const entry of series) {
    for (const point of entry.points) timestamps.add(point.timestamp);
  }

  const pointsById = new Map(
    series.map((entry) => [entry.id, new Map(entry.points.map((point) => [point.timestamp, point.value]))]),
  );

  return [...timestamps]
    .sort((left, right) => left - right)
    .map((timestamp) => {
      const row: TrendRow = { timestamp };
      for (const [id, points] of pointsById) {
        const value = points.get(timestamp);
        if (value !== undefined) row[id] = value;
      }
      return row;
    });
}
