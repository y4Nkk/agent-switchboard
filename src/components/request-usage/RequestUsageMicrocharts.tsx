import type { RequestTrendPoint } from "./request-trend";

/** Overview only; missing buckets break the line instead of implying continuity. */
export function RequestUsageSparkline({ points }: { points: RequestTrendPoint[] }) {
  const valid = points.filter((point): point is RequestTrendPoint & { value: number } => point.value !== null);
  if (valid.length === 0) return null;
  const maximum = Math.max(...valid.map((point) => point.value), 1);
  const start = points[0].atMs;
  const timeWindow = Math.max(points[points.length - 1].atMs - start, 1);
  const segments: Array<Array<{ x: number; y: number }>> = [];
  let segment: Array<{ x: number; y: number }> = [];
  for (const point of points) {
    if (point.value === null) {
      if (segment.length > 0) segments.push(segment);
      segment = [];
    } else {
      segment.push({ x: 4 + (point.atMs - start) / timeWindow * 232, y: 44 - point.value / maximum * 36 });
    }
  }
  if (segment.length > 0) segments.push(segment);
  return <svg className="asb-request-metric-sparkline" viewBox="0 0 240 48" preserveAspectRatio="none" aria-hidden="true">
    <path className="asb-request-metric-baseline" d="M4 44 H236" />
    {segments.map((line, index) => line.length === 1
      ? <circle key={index} cx={line[0].x} cy={line[0].y} r="2" />
      : <polyline key={index} points={line.map((point) => `${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(" ")} />)}
  </svg>;
}

export function RequestUsageRatioRing({ value }: { value: number | null }) {
  return <svg className="asb-request-metric-ring" data-empty={value === null} viewBox="0 0 48 48" aria-hidden="true">
    <circle className="asb-request-metric-ring-track" cx="24" cy="24" r="19" />
    {value !== null && <circle className="asb-request-metric-ring-value" cx="24" cy="24" r="19"
      pathLength="100" strokeDasharray={`${Math.min(100, Math.max(0, value))} 100`} transform="rotate(-90 24 24)" />}
  </svg>;
}
