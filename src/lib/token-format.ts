import { currentLanguage } from "../i18n/current.ts";

export const TOKEN_UNIT = "tokens";

function exactTokenFormatter(): Intl.NumberFormat {
  return new Intl.NumberFormat(currentLanguage());
}

function compactTokenFormatter(): Intl.NumberFormat {
  return new Intl.NumberFormat(currentLanguage(), { maximumFractionDigits: 1 });
}

const TOKEN_MAGNITUDES = [
  { divisor: 1_000_000_000, suffix: "B" },
  { divisor: 1_000_000, suffix: "M" },
  { divisor: 1_000, suffix: "K" },
] as const;

/** Exact, separator-delimited token value for audit-friendly detail tables. */
export function formatTokenCount(value: number): string {
  return Number.isFinite(value) ? exactTokenFormatter().format(value) : "—";
}

/** Compact K/M/B token value for scan-friendly summaries and charts. */
export function formatCompactTokenCount(value: number): string {
  if (!Number.isFinite(value)) return "—";

  const magnitude = tokenMagnitude(value);
  if (!magnitude) return exactTokenFormatter().format(value);
  return `${compactTokenFormatter().format(value / magnitude.divisor)}${magnitude.suffix}`;
}

function tokenMagnitude(value: number): (typeof TOKEN_MAGNITUDES)[number] | null {
  const absolute = Math.abs(value);
  for (const [index, magnitude] of TOKEN_MAGNITUDES.entries()) {
    if (absolute < magnitude.divisor) continue;

    // Avoid a visually awkward "1,000K" / "1,000M" at a rounding boundary.
    if (absolute / magnitude.divisor >= 999.95 && index > 0) {
      return TOKEN_MAGNITUDES[index - 1];
    }
    return magnitude;
  }
  return null;
}
