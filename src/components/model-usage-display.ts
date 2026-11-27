import type { ModelUsageReport } from "../api/model-usage";
import type { TFunction } from "../i18n";
import { TOKEN_UNIT } from "../lib/token-format";
import type { UsageTrendSeries } from "./charts/chart-data";

const ACTIVITY_WEEK_COUNT = 52;
export const DAYS_PER_WEEK = 7;
const DAY_MS = 86_400_000;

export function calendarDate(day: number): Date {
  return new Date(day * DAY_MS);
}

function dayNumber(date: string): number {
  return Date.parse(`${date}T00:00:00Z`) / DAY_MS;
}

function reportEndDay(generatedAt: string): number {
  const date = new Date(generatedAt);
  return Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / DAY_MS;
}

export function hasKnownUsageTotal(report: ModelUsageReport): boolean {
  const { sourceSessions, sessionsWithoutUsage } = report.coverage;
  return sourceSessions === 0 || sessionsWithoutUsage < sourceSessions;
}

export function usageOverview(report: ModelUsageReport) {
  const end = reportEndDay(report.generatedAt);
  let peak = 0;
  let longest = 0;
  let streak = 0;
  let previous = -Infinity;
  for (const day of report.days) {
    const date = dayNumber(day.date);
    if (!Number.isFinite(date) || date > end || day.totalTokens <= 0) continue;
    peak = Math.max(peak, day.totalTokens);
    streak = date === previous + 1 ? streak + 1 : 1;
    longest = Math.max(streak, longest);
    previous = date;
  }
  return {
    total: report.groups.reduce((sum, group) => sum + group.totalTokens, 0),
    peak,
    currentStreak: previous >= end - 1 ? streak : 0,
    longestStreak: longest,
  };
}

export function activityWeeks(report: ModelUsageReport) {
  const end = reportEndDay(report.generatedAt);
  const mondayOffset = (calendarDate(end).getUTCDay() + 6) % DAYS_PER_WEEK;
  const first = end - mondayOffset - (ACTIVITY_WEEK_COUNT - 1) * DAYS_PER_WEEK;
  const usage = new Map(report.days.map((day) => [dayNumber(day.date), day.totalTokens]));
  return Array.from({ length: ACTIVITY_WEEK_COUNT }, (_, column) => {
    const start = first + column * DAYS_PER_WEEK;
    return Array.from({ length: DAYS_PER_WEEK }, (_, row) => {
      const day = start + row;
      return { day, tokens: day <= end ? (usage.get(day) ?? 0) : null };
    });
  });
}

export interface ModelUsageSlice {
  id: string;
  label: string;
  value: number;
  models: (string | null)[];
}

const MAX_SLICES = 6;

export function modelUsageSlices(report: ModelUsageReport, t: TFunction): ModelUsageSlice[] {
  const totals = new Map<string | null, number>();
  for (const group of report.groups) {
    if (group.totalTokens > 0) totals.set(group.model, (totals.get(group.model) ?? 0) + group.totalTokens);
  }
  const ranked = [...totals].sort((left, right) => right[1] - left[1]);
  const slices = ranked.slice(0, ranked.length > MAX_SLICES ? MAX_SLICES - 1 : MAX_SLICES)
    .map(([model, value]) => ({ id: JSON.stringify(model), label: model?.trim() || t("usage.composition.unrecordedModel"),
      value, models: [model] }));
  const rest = ranked.slice(slices.length);
  if (rest.length > 0) slices.push({ id: "other", label: t("usage.composition.otherCount", { count: rest.length }),
    value: rest.reduce((sum, [, value]) => sum + value, 0), models: rest.map(([model]) => model) });
  return slices;
}

function trendDays(report: ModelUsageReport, recorded: number[]): number[] {
  if (report.range !== "all") {
    const length = report.range === "today" ? 1 : report.range === "last7Days" ? 7 : 30;
    const end = reportEndDay(report.generatedAt);
    return Array.from({ length }, (_, index) => end - length + index + 1);
  }

  // A long empty interval needs only its two zero endpoints.
  return recorded.flatMap((day, index) => {
    const gap = day - recorded[index - 1];
    if (gap > 2) return [recorded[index - 1] + 1, day - 1, day];
    return gap === 2 ? [day - 1, day] : [day];
  });
}

export function modelUsageTrend(report: ModelUsageReport, slices: ModelUsageSlice[], t: TFunction): UsageTrendSeries[] {
  const recorded = new Map(report.days.flatMap((day) => {
    const date = dayNumber(day.date);
    return Number.isFinite(date) ? [[date, day] as const] : [];
  }));
  if (recorded.size === 0) return [];
  const rows = trendDays(report, [...recorded.keys()]).map((day) => ({
    at: calendarDate(day).toISOString(),
    total: recorded.get(day)?.totalTokens ?? 0,
    models: new Map<string | null, number>(recorded.get(day)?.models?.map(({ model, totalTokens }) => [model, totalTokens])),
  }));
  if ([...recorded.values()].some((day) => day.models === null)) {
    return [{ id: "total", label: t("usage.chart.total"), unit: TOKEN_UNIT,
      points: rows.map(({ at, total }) => ({ at, value: total })) }];
  }
  return slices.map((slice, index) => ({ id: `model-${index}`, label: slice.label, unit: TOKEN_UNIT,
    points: rows.map(({ at, models }) => ({ at,
      value: slice.models.reduce((sum, model) => sum + (models.get(model) ?? 0), 0) })) }));
}
