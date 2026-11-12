export function RequestUsageBar({ value, maximum, marker = null }: {
  value: number | null; maximum: number; marker?: number | null;
}) {
  const scale = (amount: number) => `${maximum > 0 ? Math.min(100, amount / maximum * 100) : 0}%`;
  return <span className={`asb-request-usage-bar${value === null ? " is-missing" : ""}`} aria-hidden="true">
    {value !== null && <span className="asb-request-usage-bar-fill" style={{ width: scale(value) }} />}
    {value !== null && marker !== null && <span className="asb-request-usage-bar-marker" style={{ left: scale(marker) }} />}
  </span>;
}

export function RequestUsageBarScale({ end }: { end: string | null }) {
  if (end === null) return null;
  return <div className="asb-request-usage-bar-line asb-request-usage-axis" aria-hidden="true">
    <span className="asb-request-usage-axis-range"><span>0</span><span>{end}</span></span>
  </div>;
}
