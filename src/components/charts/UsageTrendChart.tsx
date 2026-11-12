import { useEffect, useId, useMemo, useState } from "react";
import { Area, ComposedChart, Line, Tooltip, XAxis, YAxis } from "recharts";
import { useCountUp } from "@/hooks/use-count-up";
import { useI18n, type TFunction } from "../../i18n";
import { ChartFrame } from "./ChartFrame";
import {
  chartSeriesColor,
  formatChartAxisTime,
  formatChartAxisValue,
  formatChartTimestamp,
  formatChartTooltipDate,
  formatChartValue,
  prepareTrendSeries,
  trendYAxisDomain,
  trendSeriesShareUnit,
  type PreparedTrendSeries,
  type UsageTrendSeries,
  type UsageTrendValueKind,
} from "./chart-data";

interface Props {
  series: UsageTrendSeries[];
  ariaLabel: string;
  emptyMessage: string;
  /** Visible heading for the standalone local-usage analysis surface. */
  title?: string;
  /** Source semantics determine formatting and vertical scale. Units alone do
   * not identify a local Token total. */
  valueKind: UsageTrendValueKind;
  /** "default" is the full showcase card for the usage page; "compact" fits
   * trend charts embedded in provider and official-quota panels. */
  size?: "default" | "compact";
}

/** Axis tick text rides the caption step; SVG text needs the literal size. */
const AXIS_TICK = { fontSize: 12, fill: "var(--asb-text-muted)" };
/** The usage trend chart card, fed by real recorded points only. It preserves
 * input point order and never fills missing timestamps with inferred values;
 * series that were read at different times simply leave gaps instead of
 * inventing points. */
export function UsageTrendChart({
  series,
  ariaLabel,
  emptyMessage,
  title,
  valueKind,
  size = "default",
}: Props) {
  const { t } = useI18n();
  // `t` re-creates per language change; including it re-resolves the series
  // labels (via the shared prepareTrendSeries) instead of keeping stale ones.
  const prepared = useMemo(() => prepareTrendSeries(series), [series, t]);

  if (prepared.length === 0) {
    return <p className="asb-chart-empty" role="status">{emptyMessage}</p>;
  }

  if (!trendSeriesShareUnit(prepared)) {
    return <p className="asb-chart-empty" role="alert">{t("usage.trend.mixedUnits")}</p>;
  }

  return (
    <TrendCard
      prepared={prepared}
      valueKind={valueKind}
      ariaLabel={ariaLabel}
      title={title}
      size={size}
    />
  );
}

function TrendCard({ prepared, valueKind, ariaLabel, title, size }: {
  prepared: PreparedTrendSeries[]; valueKind: UsageTrendValueKind; ariaLabel: string;
  title?: string; size: "default" | "compact";
}) {
  const { t } = useI18n();
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  useEffect(() => setActiveIndex(null), [prepared]);
  const rows = mergeSeriesRows(prepared);
  const unit = prepared[0].unit;
  const pointCount = prepared.reduce((total, entry) => total + entry.points.filter((point) => point.value !== null).length, 0);
  const activeRow = activeIndex !== null && activeIndex < rows.length ? rows[activeIndex] : null;
  const figure = resolveTrendFigure(rows, prepared, activeRow, valueKind === "local-token", t);
  const display = useCountUp(figure.value);
  const label = activeRow ? formatChartAxisTime(figure.timestamp) : figure.label;
  const caption = activeRow
    ? t("usage.chart.pointAtTime", { label: figure.label, time: formatChartTimestamp(figure.timestamp) })
    : unit ? t("usage.chart.unitCaption", { unit }) : t("usage.chart.pointCount", { count: pointCount });
  return <figure className={size === "compact" ? "asb-usage-chart-card is-compact" : "asb-usage-chart-card"}
    aria-label={ariaLabel}>
    {title && <figcaption className="asb-usage-chart-title">{title}</figcaption>}
    <TrendHeading label={label} display={display} unit={unit} valueKind={valueKind}
      caption={caption} prepared={prepared} activeIndex={activeIndex} />
    <div className="asb-usage-chart-body">
      <TrendPlot rows={rows} prepared={prepared} valueKind={valueKind} onActiveIndex={setActiveIndex} />
    </div>
  </figure>;
}

function TrendHeading({ label, display, unit, valueKind, caption, prepared, activeIndex }: {
  label: string; display: number; unit: string | null; valueKind: UsageTrendValueKind;
  caption: string; prepared: PreparedTrendSeries[]; activeIndex: number | null;
}) {
  return <div className="asb-usage-chart-head">
    <div className="asb-usage-chart-readout">
      <p className="asb-usage-chart-label">{label}</p>
      <p key={activeIndex ?? "rest"} className="asb-usage-chart-value">{formatChartValue(display, unit, valueKind)}</p>
      <p className="asb-usage-chart-caption">{caption}</p>
    </div>
    <dl className="asb-usage-chart-legend">
      {prepared.map((entry, index) => <div key={entry.id} className="asb-usage-chart-legend-item">
        <span className="asb-usage-chart-swatch" style={{ backgroundColor: chartSeriesColor(index) }} aria-hidden />
        <dt className="asb-usage-chart-legend-label">{entry.label}</dt>
      </div>)}
    </dl>
  </div>;
}

function TrendPlot({ rows, prepared, valueKind, onActiveIndex }: {
  rows: TrendRow[]; prepared: PreparedTrendSeries[]; valueKind: UsageTrendValueKind;
  onActiveIndex: (index: number | null) => void;
}) {
  const gradientId = useId();
  return <ChartFrame>
    <ComposedChart data={rows} margin={{ top: 4, right: 6, bottom: 0, left: 0 }}
      onMouseMove={(state) => {
        const index = Number(state.activeTooltipIndex);
        const row = rows[index];
        onActiveIndex(state.isTooltipActive && row && prepared.some((entry) => typeof row[entry.id] === "number") ? index : null);
      }} onMouseLeave={() => onActiveIndex(null)}>
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={chartSeriesColor(0)} stopOpacity={0.35} />
          <stop offset="100%" stopColor={chartSeriesColor(0)} stopOpacity={0} />
        </linearGradient>
      </defs>
      <YAxis width={44} domain={trendYAxisDomain(prepared, valueKind)} tickCount={4}
        tickFormatter={(value: number) => formatChartAxisValue(value, valueKind)}
        tickLine={false} axisLine={false} tick={AXIS_TICK} />
      <XAxis dataKey="timestamp" type="number" scale="time" domain={["dataMin", "dataMax"]}
        tickFormatter={(value: number) => formatChartAxisTime(Number(value))}
        tickLine={false} axisLine={false} tickMargin={12} tick={AXIS_TICK} />
      <Tooltip isAnimationActive={false} content={({ active, payload }) => {
        const row = payload[0]?.payload as TrendRow | undefined;
        return active && row ? <TrendTooltipCard row={row} prepared={prepared}
          unit={prepared[0].unit} valueKind={valueKind} /> : null;
      }} cursor={{ stroke: "var(--asb-hairline-strong)", strokeWidth: 1, strokeDasharray: "4 4" }} />
      {prepared.length === 1 && <Area type="monotone" dataKey={prepared[0].id}
        stroke="none" fill={`url(#${gradientId})`} dot={false} connectNulls={false} isAnimationActive={false} />}
      {prepared.map((entry, index) => <Line key={entry.id} type="monotone" dataKey={entry.id}
        name={entry.label} stroke={chartSeriesColor(index)} strokeWidth={2.5}
        dot={<IsolatedDot color={chartSeriesColor(index)} timestamps={isolatedTimestamps(rows, entry.id)} />}
        activeDot={<ActiveDot color={chartSeriesColor(index)} />} connectNulls={false} isAnimationActive={false} />)}
    </ComposedChart>
  </ChartFrame>;
}

function isolatedTimestamps(rows: TrendRow[], id: string): Set<number> {
  return new Set(rows.filter((row, index) => typeof row[id] === "number"
    && typeof rows[index - 1]?.[id] !== "number" && typeof rows[index + 1]?.[id] !== "number")
    .map((row) => row.timestamp));
}

function IsolatedDot({ color, timestamps, cx, cy, payload }: {
  color: string; timestamps: Set<number>; cx?: number; cy?: number; payload?: TrendRow;
}) {
  if (cx === undefined || cy === undefined || !payload || !timestamps.has(payload.timestamp)) return null;
  return <circle cx={cx} cy={cy} r={3} fill={color} stroke="var(--asb-content-muted)" strokeWidth={2} />;
}

/** The hover marker: a soft halo behind a solid dot ringed by the card
 * surface. Recharts clones this element with the active point's coordinates. */
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
        stroke="var(--asb-content-muted)"
        strokeWidth={2}
      />
    </g>
  );
}

/** The hover tooltip: a floating card restating the hovered column as one row
 * per series that actually recorded a point there. Series without a real
 * value are omitted, mirroring the chart's no-invented-values rule; a sum is
 * shown only for local-token trends, matching the card header. */
function TrendTooltipCard({
  row,
  prepared,
  unit,
  valueKind,
}: {
  row: TrendRow;
  prepared: PreparedTrendSeries[];
  unit: string | null;
  valueKind: UsageTrendValueKind;
}) {
  const entries = prepared
    .map((entry, index) => ({ entry, index, value: row[entry.id] }))
    .filter((item) => typeof item.value === "number");
  if (entries.length === 0) return null;

  const total =
    valueKind === "local-token" ? entries.reduce((sum, item) => sum + item.value!, 0) : null;

  return (
    <div className="asb-usage-chart-tooltip">
      <p className="asb-usage-chart-tooltip-title">
        {formatChartTooltipDate(row.timestamp)}
        {total !== null ? ` · ${formatChartValue(total, unit, valueKind)}` : null}
      </p>
      <div className="asb-usage-chart-tooltip-rows">
        {entries.map(({ entry, index, value }) => (
          <div key={entry.id} className="asb-usage-chart-tooltip-row">
            <span className="asb-usage-chart-tooltip-series">
              <span
                className="asb-usage-chart-swatch"
                style={{ backgroundColor: chartSeriesColor(index) }}
                aria-hidden
              />
              {entry.label}
            </span>
            <span className="asb-usage-chart-tooltip-value">
              {formatChartValue(value!, unit, valueKind)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

interface TrendRow {
  timestamp: number;
  [seriesId: string]: number | null | undefined;
}

interface TrendFigure {
  label: string;
  value: number;
  timestamp: number;
}

function resolveTrendFigure(
  rows: TrendRow[],
  series: PreparedTrendSeries[],
  activeRow: TrendRow | null,
  aggregateSeries: boolean,
  t: TFunction,
): TrendFigure {
  if (activeRow) return figureForRow(activeRow, series, aggregateSeries, t);

  if (aggregateSeries) {
    const latestCompleteRow = [...rows].reverse().find((row) => rowHasEverySeries(row, series));
    if (latestCompleteRow) return aggregateFigure(latestCompleteRow, series, t);
  }

  return latestObservedFigure(series);
}

function figureForRow(
  row: TrendRow,
  series: PreparedTrendSeries[],
  aggregateSeries: boolean,
  t: TFunction,
): TrendFigure {
  if (aggregateSeries && rowHasEverySeries(row, series)) return aggregateFigure(row, series, t);

  const observedSeries = series.find((entry) => typeof row[entry.id] === "number");
  if (!observedSeries) return latestObservedFigure(series);

  return {
    label: observedSeries.label,
    value: row[observedSeries.id]!,
    timestamp: row.timestamp,
  };
}

function aggregateFigure(row: TrendRow, series: PreparedTrendSeries[], t: TFunction): TrendFigure {
  return {
    label: t("usage.chart.total"),
    value: series.reduce((sum, entry) => sum + row[entry.id]!, 0),
    timestamp: row.timestamp,
  };
}

function latestObservedFigure(series: PreparedTrendSeries[]): TrendFigure {
  return series
    .flatMap((entry) => entry.points.flatMap((point) => point.value === null ? []
      : [{ label: entry.label, value: point.value, timestamp: point.timestamp }]))
    .reduce((latest, point) => (point.timestamp > latest.timestamp ? point : latest));
}

function rowHasEverySeries(row: TrendRow, series: PreparedTrendSeries[]): boolean {
  return series.every((entry) => typeof row[entry.id] === "number");
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
