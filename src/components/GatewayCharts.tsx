import { useId } from "react";
import { Area, AreaChart, CartesianGrid, Tooltip, XAxis, YAxis } from "recharts";
import type { GatewayMetricsStatus, GatewaySample, UpstreamProtocol } from "../api/client";
import { useI18n } from "../i18n";
import { protocolLabel } from "../lib/protocol";
import { formatCompactUsageValue, formatUsageValue } from "../lib/usage-format";
import { ChartFrame } from "./charts/ChartFrame";
import { FactList } from "./FactList";

const MINUTE_MS = 60_000;
const WINDOW_MINUTES = 60;
const AXIS_TICK = { fontSize: 11, fill: "var(--asb-text-muted)" };
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
    <header className="asb-gateway-charts-heading">
      <h3 className="asb-section-title" id={titleId}>{t("gateway.metricsTitle")}</h3>
      <div className="asb-gateway-totals">
        <span>{t("gateway.metricsTotalsScope")}</span>
        <FactList inline rows={[
          { label: t("gateway.totalRequests"), value: <strong>{formatUsageValue(metrics.totalRequests)}</strong> },
          { label: t("gateway.failedRequests"), value: <strong className={metrics.failedRequests > 0 ? "asb-gateway-failed-total" : undefined}>
            {formatUsageValue(metrics.failedRequests)}
          </strong> },
        ]} />
      </div>
    </header>
    <div className="asb-gateway-chart-grid">
      <RequestTrend metrics={metrics} />
      <RequestLatency samples={metrics.samples} />
      <ProtocolDistribution samples={metrics.samples} />
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
          <AreaChart data={points} title={name} accessibilityLayer margin={{ top: 12, right: 16, bottom: 0, left: 0 }}>
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
            <Tooltip isAnimationActive={false} cursor={{ stroke: "var(--asb-hairline-strong)" }}
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
  const p50 = percentile(0.5);
  const p95 = percentile(0.95);
  const step = Math.max(1, 10 ** Math.floor(Math.log10(Math.max(1, p95 ?? 0))) / 2);
  const ceiling = Math.max(2 * step, Math.ceil((p95 ?? 0) / (2 * step)) * 2 * step);
  const values = [
    { key: "P50", description: t("gateway.latencyMedian"), value: p50 },
    { key: "P95", description: t("gateway.latencyP95"), value: p95 },
  ];
  return <figure className="asb-gateway-chart is-latency">
    <figcaption className="asb-gateway-chart-title">
      <strong>{t("gateway.latencyTitle")}</strong>
      {durations.length > 0 && <span>{t("gateway.chartSampleCount", { count: formatUsageValue(durations.length) })}</span>}
    </figcaption>
    {durations.length === 0 ? <p className="asb-gateway-chart-empty">{t("gateway.latencyNoCompleted")}</p> : (
      <div className="asb-gateway-latency-chart">
        <dl className="asb-gateway-latency-values">
          {values.map(({ key, description, value }) => (
            <div key={key}>
              <dt><abbr title={description}>{key}</abbr></dt>
              <dd>{formatUsageValue(value, "ms")}</dd>
            </div>
          ))}
        </dl>
        <div className="asb-gateway-latency-plot" aria-hidden="true">
          <div className="asb-gateway-latency-grid">
            {values.map(({ key, value }) => <span key={key} className="asb-gateway-latency-point"
              style={{ insetInlineStart: `${(value / ceiling) * 100}%` }} />)}
          </div>
          <div className="asb-gateway-latency-ticks">
            <span>0</span><span>{formatCompactUsageValue(ceiling / 2)}</span><span>{formatCompactUsageValue(ceiling)} ms</span>
          </div>
        </div>
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
  let offset = 0;
  const slices = entries.map(([protocol, count]) => {
    const proportion = count / total * 100;
    const slice = { protocol, count, proportion, offset };
    offset += proportion;
    return slice;
  });
  return <figure className="asb-gateway-chart is-protocol">
    <figcaption className="asb-gateway-chart-title"><strong>{t("gateway.protocolTitle")}</strong></figcaption>
    {total === 0 ? <p className="asb-gateway-chart-empty">{t("gateway.protocolEmpty")}</p> : <>
      <div className="asb-gateway-donut">
        <svg viewBox="0 0 100 100" aria-hidden="true" focusable="false">
          <circle className="asb-gateway-donut-track" cx="50" cy="50" r="40" />
          {slices.map(({ protocol, proportion, offset }) => (
            <circle key={protocol} cx="50" cy="50" r="40" pathLength="100" transform="rotate(-90 50 50)"
              stroke={PROTOCOL_TONES[protocol]} strokeDasharray={`${proportion} ${100 - proportion}`} strokeDashoffset={-offset} />
          ))}
        </svg>
        <div className="asb-gateway-donut-center">
          <strong>{formatUsageValue(total)}</strong><span>{t("gateway.chartSamples")}</span>
        </div>
      </div>
      <ul className="asb-gateway-protocol-key">
        {slices.map(({ protocol, count, proportion }) => <li key={protocol}>
          <i style={{ backgroundColor: PROTOCOL_TONES[protocol] }} aria-hidden="true" />
          <span>{protocolLabel(t, protocol)}</span>
          <span className="asb-gateway-protocol-share">
            {formatUsageValue(count)} <span aria-hidden="true">·</span> {formatUsageValue(proportion, "%")}
          </span>
        </li>)}
      </ul>
    </>}
  </figure>;
}
