import assert from "node:assert/strict";
import { test } from "node:test";
import { requestTrendPoints } from "../src/components/request-usage/request-trend.ts";
import { prepareTrendSeries, trendYAxisDomain } from "../src/components/charts/chart-data.ts";
import type { RequestUsageReport } from "../src/api/request-usage.ts";

function bucket(atMs: number, value: number | null): RequestUsageReport["trend"][number] {
  return { atMs, speedP50: value, firstTokenP50Ms: null, durationP50Ms: null, failureRate: null };
}

test("sparse request history breaks at missing buckets and retains real zero values", () => {
  const points = requestTrendPoints({ bucketMs: 1000, trend: [bucket(0, 25), bucket(3000, 0)] }, "speedP50");
  assert.deepEqual(points, [
    { atMs: 0, value: 25 }, { atMs: 1000, value: null }, { atMs: 3000, value: 0 },
  ]);
});

test("an observed bucket with an unknown metric remains a gap", () => {
  assert.deepEqual(requestTrendPoints({ bucketMs: 1000,
    trend: [bucket(0, 25), bucket(1000, null), bucket(2000, 32)] }, "speedP50"), [
    { atMs: 0, value: 25 }, { atMs: 1000, value: null }, { atMs: 2000, value: 32 },
  ]);
});

test("long unrecorded periods do not allocate one placeholder per absent bucket", () => {
  const points = requestTrendPoints({ bucketMs: 60_000,
    trend: [bucket(0, 25), bucket(10 * 365 * 86_400_000, 32)] }, "speedP50");
  assert.equal(points.length, 3);
  assert.equal(points[1].value, null);
  assert.deepEqual(requestTrendPoints({ bucketMs: 60_000, trend: [] }, "durationP50Ms"), []);
});

test("shared trend preparation preserves explicit gaps and zero, without fabricating an empty series", () => {
  const prepared = prepareTrendSeries([{ id: "speed", label: "Speed", points: [
    { at: "2026-11-12T00:00:00Z", value: 25 },
    { at: "2026-11-12T00:01:00Z", value: null },
    { at: "2026-11-12T00:02:00Z", value: 0 },
  ] }]);
  assert.deepEqual(prepared[0].points.map((point) => point.value), [25, null, 0]);
  const [minimum, maximum] = trendYAxisDomain(prepared, "generic");
  assert.equal(minimum, 0);
  assert.ok(Number.isFinite(maximum) && maximum >= 25, "the scale must include every observed value");
  assert.deepEqual(trendYAxisDomain(prepared, "percentage"), [0, 100]);
  assert.deepEqual(prepareTrendSeries([{ id: "empty", label: "Empty", points: [
    { at: "2026-11-12T00:00:00Z", value: null },
  ] }]), []);
});
