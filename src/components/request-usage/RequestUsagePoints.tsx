import type { CSSProperties } from "react";

export function RequestUsagePoints({ value, maximum, marker = null }: {
  value: number | null; maximum: number; marker?: number | null;
}) {
  const position = (amount: number): CSSProperties => ({
    "--asb-request-point-position": maximum > 0 ? Math.max(0, Math.min(amount / maximum, 1)) * 100 + "%" : "0%",
  } as CSSProperties);
  return <span className="asb-request-usage-points" aria-hidden="true">
    {value !== null && <span className="asb-request-usage-point" style={position(value)} />}
    {marker !== null && <span className="asb-request-usage-point is-companion" style={position(marker)} />}
  </span>;
}

export function RequestUsagePointAxis({ end }: { end: string | null }) {
  if (end === null) return null;
  return <div className="asb-request-usage-observation asb-request-usage-axis" aria-hidden="true">
    <span className="asb-request-usage-axis-range"><span>0</span><span>{end}</span></span>
  </div>;
}
