import type { RequestUsageReport } from "../../api/request-usage";
import type { RequestUsageTrend } from "./chart-metrics";

export interface RequestTrendPoint { atMs: number; value: number | null }

/** The report is sparse; one null marker breaks any run of unreported buckets. */
export function requestTrendPoints(report: Pick<RequestUsageReport, "trend" | "bucketMs">, metric: RequestUsageTrend): RequestTrendPoint[] {
  const points: RequestTrendPoint[] = [];
  let previousAt: number | null = null;
  for (const bucket of report.trend) {
    if (previousAt !== null && bucket.atMs - previousAt > report.bucketMs) {
      points.push({ atMs: previousAt + report.bucketMs, value: null });
    }
    points.push({ atMs: bucket.atMs, value: bucket[metric] });
    previousAt = bucket.atMs;
  }
  return points;
}
