import { formatCompactUsageValue, formatUsageValue } from "../../lib/usage-format.ts";
import { formatCompactTokenCount, formatTokenValue } from "../../lib/token-format.ts";
import { currentLanguage, tr } from "../../i18n/current.ts";

export interface UsageTrendPoint {
  at: string;
  value: number | null;
}

export interface UsageTrendSeries {
  id: string;
  label: string;
  unit?: string | null;
  points: UsageTrendPoint[];
}

export interface ModelUsageDistributionItem {
  id: string;
  label: string;
  value: number;
}

export interface PreparedTrendPoint extends UsageTrendPoint {
  timestamp: number;
  index: number;
}

export interface PreparedTrendSeries {
  id: string;
  label: string;
  unit: string | null;
  points: PreparedTrendPoint[];
}

/** The source contract of a trend's values. A unit string alone cannot decide
 * whether a `tokens` reading is local model usage or a provider-owned value. */
export type UsageTrendValueKind = "generic" | "local-token" | "percentage";

/** Null is an explicit gap; invalid numbers and timestamps are not observations. */
export function prepareTrendSeries(series: UsageTrendSeries[]): PreparedTrendSeries[] {
  return series.flatMap((entry, seriesIndex) => {
    const points = entry.points.flatMap((point, pointIndex) => {
      const timestamp = Date.parse(point.at);
      if (!Number.isFinite(timestamp) || (point.value !== null && !Number.isFinite(point.value))) return [];
      return [{ ...point, timestamp, index: pointIndex }];
    });
    if (!points.some((point) => point.value !== null)) return [];

    return [{
      id: entry.id.trim() || `series-${seriesIndex}`,
      label: entry.label.trim() || tr("usage.chart.unnamedSeries"),
      unit: normalizedUnit(entry.unit),
      points,
    }];
  });
}

export function trendSeriesShareUnit(series: PreparedTrendSeries[]): boolean {
  return new Set(series.map((entry) => entry.unit ?? "")).size <= 1;
}

/** Chart cards compact local token values, while all other series retain the
 * generic source-value formatter used by their adjacent ledgers. */
export function formatChartValue(
  value: number,
  unit: string | null | undefined,
  valueKind: UsageTrendValueKind,
): string {
  return valueKind === "local-token" ? formatTokenValue(value) : formatUsageValue(value, unit);
}

/** A compact tick label; token axes use the app-wide K/M/B token contract. */
export function formatChartAxisValue(value: number, valueKind: UsageTrendValueKind): string {
  return valueKind === "local-token" ? formatCompactTokenCount(value) : formatCompactUsageValue(value);
}

/** Percentages have an explicit 0–100 semantic range. Other quantities retain
 * a zero baseline when possible, while negative source values remain visible
 * instead of being clipped below the chart. */
export function trendYAxisDomain(
  series: PreparedTrendSeries[],
  valueKind: UsageTrendValueKind,
): [number, number] {
  if (valueKind === "percentage") return [0, 100];

  const values = series.flatMap((entry) => entry.points.flatMap((point) => point.value === null ? [] : [point.value]));
  const minimum = Math.min(...values);
  const maximum = Math.max(...values);

  if (minimum >= 0) return [0, Math.max(maximum, 1) * 1.1];
  if (maximum <= 0) return [Math.min(minimum, -1) * 1.1, 0];

  const padding = (maximum - minimum) * 0.1;
  return [minimum - padding, maximum + padding];
}

/** A compact local-time axis label. Full timestamps remain available on each
 * keyboard-focusable point. */
export function formatChartAxisTime(timestamp: number): string {
  const date = new Date(timestamp);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${month}/${day}`;
}

/** A human day label ("9月15日" / "Sep 15") for hover tooltips; unlike tick
 * labels it keeps no zero padding. */
export function formatChartTooltipDate(timestamp: number): string {
  return new Intl.DateTimeFormat(currentLanguage(), { month: "short", day: "numeric" })
    .format(new Date(timestamp));
}

export function formatChartTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(currentLanguage(), { dateStyle: "medium", timeStyle: "short" })
    .format(new Date(timestamp));
}

/** The categorical hue for the nth series, shared by every usage chart.
 * Series tones resolve straight to app palette tokens; the fifth falls back
 * to muted text so a fifth series still reads as "lesser". */
const SERIES_COLORS = [
  "var(--asb-action)",
  "var(--asb-claude)",
  "var(--asb-safe)",
  "var(--asb-warning)",
  "var(--asb-text-muted)",
] as const;

export function chartSeriesColor(index: number): string {
  return SERIES_COLORS[index % SERIES_COLORS.length];
}

function normalizedUnit(unit: string | null | undefined): string | null {
  const value = unit?.trim();
  return value ? value : null;
}
