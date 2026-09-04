import { formatCompactTokenCount, formatTokenValue, isTokenUnit } from "../../lib/token-format";

export interface UsageTrendPoint {
  at: string;
  value: number;
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

export const CHART_TONE_COUNT = 5;

const valueFormatter = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });
const compactFormatter = new Intl.NumberFormat("zh-CN", { notation: "compact", maximumFractionDigits: 1 });

/** Keep renderer data defensive: malformed points disappear rather than
 * creating an invalid SVG path or a made-up replacement value. */
export function prepareTrendSeries(series: UsageTrendSeries[]): PreparedTrendSeries[] {
  return series.flatMap((entry, seriesIndex) => {
    const points = entry.points.flatMap((point, pointIndex) => {
      const timestamp = Date.parse(point.at);
      if (!Number.isFinite(timestamp) || !Number.isFinite(point.value)) return [];
      return [{ ...point, timestamp, index: pointIndex }];
    });
    if (points.length === 0) return [];

    return [{
      id: entry.id.trim() || `series-${seriesIndex}`,
      label: entry.label.trim() || "未命名系列",
      unit: normalizedUnit(entry.unit),
      points,
    }];
  });
}

export function trendSeriesShareUnit(series: PreparedTrendSeries[]): boolean {
  return new Set(series.map((entry) => entry.unit ?? "")).size <= 1;
}

export function formatChartValue(value: number, unit?: string | null): string {
  if (!Number.isFinite(value)) return "—";
  if (isTokenUnit(unit)) return formatTokenValue(value);
  const formatted = valueFormatter.format(value);
  return unit === null || unit === undefined || unit.trim() === "" ? formatted : `${formatted} ${unit}`;
}

/** A compact tick label; token axes use the app-wide K/M/B token contract. */
export function formatChartAxisValue(value: number, unit?: string | null): string {
  if (!Number.isFinite(value)) return "—";
  return isTokenUnit(unit) ? formatCompactTokenCount(value) : compactFormatter.format(value);
}

/** A compact local-time axis label. Full timestamps remain available on each
 * keyboard-focusable point. */
export function formatChartAxisTime(timestamp: number): string {
  const date = new Date(timestamp);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${month}/${day}`;
}

export function formatChartTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  return `${year}年${month}月${day}日 ${hour}:${minute}`;
}

export function chartTone(index: number): string {
  return String(index % CHART_TONE_COUNT);
}

function normalizedUnit(unit: string | null | undefined): string | null {
  const value = unit?.trim();
  return value ? value : null;
}
