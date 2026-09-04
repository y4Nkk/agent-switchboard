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
