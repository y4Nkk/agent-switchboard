import { useEffect, useId, useMemo, useState } from "react";
import { Area, CartesianGrid, ComposedChart, Line, Tooltip, XAxis, YAxis } from "recharts";
import { useCountUp } from "@/hooks/use-count-up";
import { useI18n } from "../../i18n";
import { formatCompactUsageValue, formatUsageValue } from "../../lib/usage-format";
import { ChartFrame } from "./ChartFrame";
import {
  chartSeriesColor,
  formatChartAxisTime,
  formatChartTimestamp,
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
    if (valueKind === "local-token") return <figure className="asb-usage-chart-card is-empty" aria-label={ariaLabel}>
      <figcaption className="asb-usage-chart-title">{title}</figcaption>
      <p className="asb-consumption-empty" role="status">{emptyMessage}</p>
    </figure>;
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
  const local = valueKind === "local-token";
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  useEffect(() => setActiveIndex(null), [prepared]);
  const rows = useMemo(() => mergeSeriesRows(prepared), [prepared]);
  const activeRow = activeIndex !== null && activeIndex < rows.length ? rows[activeIndex] : null;
  return <figure className={size === "compact" ? "asb-usage-chart-card is-compact" : "asb-usage-chart-card"}
    aria-label={ariaLabel}>
    {title && <figcaption className="asb-usage-chart-title">{title}</figcaption>}
    {local ? <TrendLegend prepared={prepared} valueKind={valueKind} />
      : <TrendHeading prepared={prepared} activeRow={activeRow} activeIndex={activeIndex} valueKind={valueKind} />}
    <div className="asb-usage-chart-body">
      <TrendPlot rows={rows} prepared={prepared} valueKind={valueKind} onActiveIndex={local ? undefined : setActiveIndex} />
    </div>
  </figure>;
}

function TrendHeading({ activeRow, valueKind, prepared, activeIndex }: {
  activeRow: TrendRow | null; valueKind: UsageTrendValueKind; prepared: PreparedTrendSeries[]; activeIndex: number | null;
}) {
  const figure = resolveTrendFigure(prepared, activeRow);
  const display = useCountUp(figure.value);
  const unit = prepared[0].unit;
  return <div className="asb-usage-chart-head">
    <div className="asb-usage-chart-readout">
      <p className="asb-usage-chart-label">{figure.label}</p>
      <p key={activeIndex ?? "rest"} className="asb-usage-chart-value">{formatUsageValue(display, unit)}</p>
    </div>
    {prepared.length > 1 && <TrendLegend prepared={prepared} valueKind={valueKind} />}
  </div>;
}

function TrendLegend({ prepared, valueKind }: { prepared: PreparedTrendSeries[]; valueKind: UsageTrendValueKind }) {
  return <ul className="asb-usage-chart-legend">
    {prepared.map((entry, index) => <li key={entry.id} className="asb-usage-chart-legend-item">
      <span className="asb-usage-chart-swatch" style={{ backgroundColor: chartSeriesColor(index, valueKind) }} aria-hidden />
      <span className="asb-usage-chart-legend-label">{entry.label}</span>
    </li>)}
  </ul>;
}

function TrendPlot({ rows, prepared, valueKind, onActiveIndex }: {
  rows: TrendRow[]; prepared: PreparedTrendSeries[]; valueKind: UsageTrendValueKind;
  onActiveIndex?: (index: number | null) => void;
}) {
  const { language } = useI18n();
  const local = valueKind === "local-token";
  const gradientId = useId();
  const date = useMemo(() => new Intl.DateTimeFormat(language, { month: "short", day: "numeric", timeZone: "UTC" }), [language]);
  const shortRange = rows[rows.length - 1].timestamp - rows[0].timestamp <= 36 * 60 * 60 * 1000;
  const clock = useMemo(() => new Intl.DateTimeFormat(language, { hour: "2-digit", minute: "2-digit" }), [language]);
  const formatTick = (timestamp: number) => local ? date.format(timestamp)
    : shortRange ? `${formatChartAxisTime(timestamp)} ${clock.format(timestamp)}` : formatChartAxisTime(timestamp);
  const ticks = local ? (rows.length <= 8 ? rows.map((row) => row.timestamp) : undefined) : trendTimeTicks(rows, formatTick);
  return <ChartFrame>
    <ComposedChart data={rows} margin={{ top: 4, right: local ? 22 : shortRange ? 36 : 18, bottom: 0, left: local ? 22 : 0 }}
      onMouseMove={(state) => {
        if (!onActiveIndex) return;
        const index = Number(state.activeTooltipIndex);
        const row = rows[index];
        onActiveIndex(state.isTooltipActive && row && prepared.some((entry) => typeof row[entry.id] === "number") ? index : null);
      }} onMouseLeave={() => onActiveIndex?.(null)}>
      {!local && <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={chartSeriesColor(0, valueKind)} stopOpacity={0.35} />
          <stop offset="100%" stopColor={chartSeriesColor(0, valueKind)} stopOpacity={0} />
        </linearGradient>
      </defs>}
      <YAxis hide={local} width={44} domain={trendYAxisDomain(prepared, valueKind)} tickCount={4}
        tickFormatter={(value: number) => formatCompactUsageValue(value)}
        tickLine={false} axisLine={false} tick={AXIS_TICK} />
      <XAxis dataKey="timestamp" type="number" scale={local ? "utc" : "time"} domain={["dataMin", "dataMax"]}
        ticks={ticks} interval="preserveStartEnd"
        tickFormatter={(value: number) => formatTick(Number(value))}
        tickLine={false} axisLine={false} tickMargin={12} tick={AXIS_TICK} />
      {local && <CartesianGrid vertical={false} stroke="var(--asb-hairline)" strokeDasharray="3 4" />}
      <Tooltip isAnimationActive={false} content={({ active, payload }) => {
        const row = payload[0]?.payload as TrendRow | undefined;
        return active && row ? <TrendTooltipCard row={row} prepared={prepared}
          unit={prepared[0].unit} valueKind={valueKind} /> : null;
      }} cursor={{ stroke: "var(--asb-hairline-strong)", strokeWidth: 1, strokeDasharray: "4 4" }} />
      {!local && prepared.length === 1 && <Area type="monotone" dataKey={prepared[0].id}
        stroke="none" fill={`url(#${gradientId})`} dot={false} connectNulls={false} isAnimationActive={false} />}
      {prepared.map((entry, index) => <Line key={entry.id} type="monotone" dataKey={entry.id}
        name={entry.label} stroke={chartSeriesColor(index, valueKind)} strokeWidth={local ? 2 : 2.5}
        dot={local && rows.length > 1 ? false
          : <IsolatedDot color={chartSeriesColor(index, valueKind)} timestamps={isolatedTimestamps(rows, entry.id)} />}
        activeDot={<ActiveDot color={chartSeriesColor(index, valueKind)} />} connectNulls={false} isAnimationActive={false} />)}
    </ComposedChart>
  </ChartFrame>;
}

function trendTimeTicks(rows: TrendRow[], format: (timestamp: number) => string): number[] {
  const labels = new Set<string>();
  const uniqueTicks = rows.flatMap(({ timestamp }) => {
    const label = format(timestamp);
    if (labels.has(label)) return [];
    labels.add(label);
    return [timestamp];
  });
  const maxTicks = 7;
  if (uniqueTicks.length <= maxTicks) return uniqueTicks;
  return Array.from({ length: maxTicks }, (_, index) => uniqueTicks[Math.round(index * (uniqueTicks.length - 1) / (maxTicks - 1))]);
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

/** Provider series preserve gaps. Model series share a calendar and can be added. */
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
  const { language } = useI18n();
  const entries = prepared
    .map((entry, index) => ({ entry, index, value: row[entry.id] }))
    .filter((item) => typeof item.value === "number");
  if (entries.length === 0) return null;

  const local = valueKind === "local-token";
  const total = local ? entries.reduce((sum, item) => sum + item.value!, 0) : null;
  const date = local ? new Intl.DateTimeFormat(language, { dateStyle: "medium", timeZone: "UTC" }).format(row.timestamp)
    : formatChartTimestamp(row.timestamp);

  return (
    <div className="asb-usage-chart-tooltip">
      <p className="asb-usage-chart-tooltip-title">
        {date}
        {total !== null && (entries.length > 1 ? ` · ${formatUsageValue(total, unit)}` : null)}
      </p>
      <div className="asb-usage-chart-tooltip-rows">
        {entries.map(({ entry, index, value }) => (
          <div key={entry.id} className="asb-usage-chart-tooltip-row">
            <span className="asb-usage-chart-tooltip-series">
              <span
                className="asb-usage-chart-swatch"
                style={{ backgroundColor: chartSeriesColor(index, valueKind) }}
                aria-hidden
              />
              {entry.label}
            </span>
            <span className="asb-usage-chart-tooltip-value">
              {formatUsageValue(value, unit)}
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
  series: PreparedTrendSeries[],
  activeRow: TrendRow | null,
): TrendFigure {
  if (activeRow) {
    for (const entry of series) {
      const value = activeRow[entry.id];
      if (typeof value === "number") return { label: entry.label, value, timestamp: activeRow.timestamp };
    }
  }
  return latestObservedFigure(series);
}

function latestObservedFigure(series: PreparedTrendSeries[]): TrendFigure {
  return series
    .flatMap((entry) => entry.points.flatMap((point) => point.value === null ? []
      : [{ label: entry.label, value: point.value, timestamp: point.timestamp }]))
    .reduce((latest, point) => (point.timestamp > latest.timestamp ? point : latest));
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
