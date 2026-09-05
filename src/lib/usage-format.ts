import type { UsageReading, UsageSummary } from "../api/client";

const exactValueFormatter = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });
const compactValueFormatter = new Intl.NumberFormat("zh-CN", {
  notation: "compact",
  maximumFractionDigits: 1,
});

/** Formats a generic usage value with the unit carried by its source contract.
 * This keeps table values audit-friendly and leaves token-specific compaction
 * to the local-model token display contract. */
export function formatUsageValue(value: number | null | undefined, unit?: string | null): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return appendUnit(exactValueFormatter.format(value), unit);
}

/** Compact axis-scale value. The chart caption owns the unit label, so this
 * intentionally returns only the scaled number rather than duplicating it on
 * every tick. */
export function formatCompactUsageValue(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return compactValueFormatter.format(value);
}

function appendUnit(value: string, unit: string | null | undefined): string {
  const normalized = unit?.trim();
  return normalized ? `${value} ${normalized}` : value;
}

export function usageProgress(reading: UsageReading): number | null {
  if (reading.total === null || !Number.isFinite(reading.total) || reading.total <= 0) return null;
  const used = reading.used ?? (reading.remaining === null ? null : reading.total - reading.remaining);
  if (used === null || !Number.isFinite(used)) return null;
  return (used / reading.total) * 100;
}

/** Keep plans and units separate; a balance alone cannot imply a percentage. */
export function formatUsageSummary(summary: UsageSummary): string {
  if (summary.readings.length === 0) return "暂无额度读数";
  return summary.readings.map((reading, index) => {
    const usedPercent = usageProgress(reading);
    const parts: string[] = [];
    if (usedPercent !== null) parts.push(`剩余 ${exactValueFormatter.format(100 - usedPercent)}%`);
    if (reading.remaining !== null) parts.push(`余额 ${formatUsageValue(reading.remaining, reading.unit)}`);
    else if (usedPercent === null && reading.used !== null) parts.push(`已用 ${formatUsageValue(reading.used, reading.unit)}`);
    if (parts.length === 0) parts.push("暂无额度读数");
    const name = reading.planName?.trim() || (summary.readings.length > 1 ? `额度 ${index + 1}` : "");
    return `${name ? `${name}：` : ""}${parts.join(" · ")}`;
  }).join("；");
}
