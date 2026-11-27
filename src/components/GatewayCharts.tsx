import { useId } from "react";
import { Area, AreaChart, CartesianGrid, Tooltip as ChartTooltip, XAxis, YAxis } from "recharts";
import type { GatewayMetricsStatus, GatewaySample, UpstreamProtocol } from "../api/client";
import { useI18n } from "../i18n";
import { protocolLabel } from "../lib/protocol";
import { formatCompactUsageValue, formatUsageValue } from "../lib/usage-format";
import { ChartFrame } from "./charts/ChartFrame";
import { FactList, type FactListRow } from "./FactList";
import { CompositionRing } from "./charts/CompositionRing";
import { Tooltip } from "./Tooltip";

const MINUTE_MS = 60_000;
const WINDOW_MINUTES = 60;
const AXIS_TICK = { fontSize: 11, fill: "var(--asb-text-muted)" };
const LATENCY_RANGES = [
  { upperMs: 250, label: "<250 ms", color: "color-mix(in srgb, var(--asb-action) 57%, var(--asb-content))" },
  { upperMs: 500, label: "250–<500 ms", color: "var(--asb-action)" },
  { upperMs: 1000, label: "0.5–<1 s", color: "color-mix(in srgb, var(--asb-action) 44%, var(--asb-claude))" },
  { upperMs: 2000, label: "1–<2 s", color: "var(--asb-claude)" },
  { upperMs: Infinity, label: "≥2 s", color: "color-mix(in srgb, var(--asb-claude) 55%, var(--asb-text))" },
];
const PROTOCOL_TONES: Record<UpstreamProtocol, string> = {
  responses: "var(--asb-action)",
  chatCompletions: "var(--asb-claude)",
  anthropicMessages: "var(--asb-safe)",
  geminiGenerateContent: "var(--asb-warning)",
};

export function GatewayCharts({ metrics }: { metrics: GatewayMetricsStatus }) {
  const { t } = useI18n();
  const titleId = useId();
  return <section className="asb-gateway-charts" aria-labelledby={titleId}>
    <h4 className="asb-group-title" id={titleId}>{t("gateway.metricsTitle")}</h4>
    <div className="asb-gateway-chart-grid">
      <RequestTrend metrics={metrics} />
      <RequestLatency samples={metrics.samples} />
      <ProtocolDistribution samples={metrics.samples} />
      <RequestOutcomes total={metrics.totalRequests} failed={metrics.failedRequests} />
    </div>
  </section>;
}

interface MinutePoint {
  atMs: number;
  count: number | null;
}

function buildTrend({ startedAtMs, totalRequests, samples }: GatewayMetricsStatus): MinutePoint[] {
  const end = Math.floor(Date.now() / MINUTE_MS);
  const start = end - WINDOW_MINUTES + 1;
  const oldest = totalRequests > samples.length && samples.length > 0
    ? Math.min(...samples.map((sample) => sample.atMs))
    : startedAtMs;
  const observedFrom = Math.floor(Math.max(oldest, startedAtMs) / MINUTE_MS);
  const points: MinutePoint[] = Array.from({ length: WINDOW_MINUTES }, (_, index) => ({
    atMs: (start + index) * MINUTE_MS,
    count: start + index < observedFrom ? null : 0,
  }));
  for (const { atMs } of samples) {
    const point = points[Math.floor(atMs / MINUTE_MS) - start];
    if (point?.count != null) point.count += 1;
  }
  return points;
}

function RequestTrend({ metrics }: { metrics: GatewayMetricsStatus }) {
  const { t, language } = useI18n();
  const gradient = useId();
  const points = buildTrend(metrics);
  const count = points.reduce((sum, point) => sum + (point.count ?? 0), 0);
  const singlePoint = points.filter((point) => point.count !== null).length === 1;
  const time = new Intl.DateTimeFormat(language, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
  const first = points[0].atMs;
  const last = points[points.length - 1].atMs;
  const name = t("gateway.trendTitle");
  return <figure className="asb-gateway-chart is-trend">
    <figcaption className="asb-gateway-chart-title">
      <strong>{name}</strong>
      {count > 0 && <span>{t("gateway.chartSampleCount", { count: formatUsageValue(count) })}</span>}
    </figcaption>
    {count === 0 ? <p className="asb-gateway-chart-empty">{t("gateway.trendEmpty")}</p> : (
      <div className="asb-gateway-trend-plot">
        <ChartFrame>
          <AreaChart data={points} aria-label={name} accessibilityLayer margin={{ top: 12, right: 16, bottom: 0, left: 0 }}>
            <defs>
              <linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1">
                <stop stopColor="var(--asb-action)" stopOpacity={0.34} />
                <stop offset="1" stopColor="var(--asb-action)" stopOpacity={0.03} />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} stroke="var(--asb-hairline)" />
            <YAxis width={34} allowDecimals={false} tickCount={4} tick={AXIS_TICK} tickSize={0}
              axisLine={false} tickMargin={6} tickFormatter={formatCompactUsageValue} />
            <XAxis dataKey="atMs" type="number" domain={[first, last]} ticks={[first, points[30].atMs, last]} tick={AXIS_TICK}
              axisLine={false} tickLine={false} tickMargin={8} tickFormatter={(atMs: number) => time.format(atMs)} />
            <ChartTooltip isAnimationActive={false} cursor={{ stroke: "var(--asb-hairline-strong)" }}
              content={({ active, payload }) => {
                const point = payload[0]?.payload as MinutePoint | undefined;
                return active && point?.count != null ? <p className="asb-chart-tooltip">
                  <time>{time.format(point.atMs)}</time>
                  <span>{t("gateway.chartSampleCount", { count: formatUsageValue(point.count) })}</span>
                </p> : null;
              }} />
            <Area type="step" dataKey="count" stroke="var(--asb-link)" strokeWidth={2}
              dot={singlePoint ? { r: 3, fill: "var(--asb-link)", stroke: "var(--asb-content-muted)" } : false}
              fill={`url(#${gradient})`} connectNulls={false} isAnimationActive={false} />
          </AreaChart>
        </ChartFrame>
      </div>
    )}
  </figure>;
}

function RequestLatency({ samples }: { samples: GatewaySample[] }) {
  const { t } = useI18n();
  const durations = samples.filter(({ status }) => status !== null)
    .map(({ durationMs }) => durationMs).sort((a, b) => a - b);
  const percentile = (fraction: number) => durations[Math.ceil(durations.length * fraction) - 1];
  const values = [
    { key: "P50", description: t("gateway.latencyMedian"), value: percentile(0.5) },
    { key: "P95", description: t("gateway.latencyP95"), value: percentile(0.95) },
  ];
  const ranges = LATENCY_RANGES.map((range) => ({ ...range, count: 0 }));
  for (const duration of durations) {
    const index = ranges.findIndex(({ upperMs }) => duration < upperMs);
    ranges[index].count += 1;
  }
  return <figure className="asb-gateway-chart is-latency">
    <figcaption className="asb-gateway-chart-title">
      <strong>{t("gateway.latencyTitle")}</strong>
    </figcaption>
    {durations.length === 0 ? <p className="asb-gateway-chart-empty">{t("gateway.latencyNoCompleted")}</p> : (
      <div className="asb-gateway-latency-chart">
        <RequestDonut total={durations.length} totalLabel={t("gateway.chartSamples")} parts={ranges} />
        <FactList inline rows={values.map<FactListRow>(({ key, description, value }) => ({
          label: <Tooltip asChild label={description}><abbr aria-label={description}>{key}</abbr></Tooltip>,
          value: formatUsageValue(value, "ms"),
          format: "number",
        }))} />
      </div>
    )}
  </figure>;
}

function ProtocolDistribution({ samples }: { samples: GatewaySample[] }) {
  const { t } = useI18n();
  const counts = new Map<UpstreamProtocol, number>();
  for (const { upstreamProtocol } of samples) {
    if (upstreamProtocol !== null) counts.set(upstreamProtocol, (counts.get(upstreamProtocol) ?? 0) + 1);
  }
  const entries = [...counts].sort((a, b) => b[1] - a[1]);
  const total = entries.reduce((sum, [, count]) => sum + count, 0);
  return <figure className="asb-gateway-chart is-protocol">
    <figcaption className="asb-gateway-chart-title"><strong>{t("gateway.protocolTitle")}</strong></figcaption>
    {total === 0 ? <p className="asb-gateway-chart-empty">{t("gateway.protocolEmpty")}</p>
      : <RequestDonut total={total} totalLabel={t("gateway.chartSamples")}
        parts={entries.map(([protocol, count]) => ({ label: protocolLabel(t, protocol), count, color: PROTOCOL_TONES[protocol] }))} />}
  </figure>;
}

function RequestOutcomes({ total, failed }: { total: number; failed: number }) {
  const { t } = useI18n();
  return <figure className="asb-gateway-chart is-outcomes">
    <figcaption className="asb-gateway-chart-title"><strong>{t("gateway.requestOutcomes")}</strong></figcaption>
    <RequestDonut total={total} totalLabel={t("gateway.totalRequests")} parts={[
      { label: t("gateway.succeededRequests"), count: total - failed, color: "var(--asb-safe)" },
      { label: t("gateway.failedRequests"), count: failed, color: "var(--asb-danger)" },
    ]} />
  </figure>;
}

function RequestDonut({ total, totalLabel, parts }: {
  total: number;
  totalLabel: string;
  parts: { label: string; count: number; color: string }[];
}) {
  return <div className="asb-gateway-breakdown">
    <div className="asb-gateway-donut">
      <CompositionRing parts={parts.map(({ label, color, count }) => ({ id: label, color, value: count }))} />
      <div className="asb-gateway-donut-center">
        <strong>{formatUsageValue(total)}</strong><span>{totalLabel}</span>
      </div>
    </div>
    <ul className="asb-gateway-breakdown-key">
      {parts.map(({ label, count, color }) => <li key={label}>
        <i style={{ backgroundColor: color }} aria-hidden="true" />
        <span>{label}</span>
        <span className="asb-gateway-breakdown-share">
          {formatUsageValue(count)} <span aria-hidden="true">·</span> {formatUsageValue(total > 0 ? count / total * 100 : 0, "%")}
        </span>
      </li>)}
    </ul>
  </div>;
}
