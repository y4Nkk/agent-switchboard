import type { GatewayMetricsStatus, GatewaySample, UpstreamProtocol } from "../api/client";
import { useI18n, type TFunction } from "../i18n";
import { isProtocolTranslation, protocolLabel } from "../lib/protocol";
import { clientFullName } from "../lib/client-name";
import { Table, type TableColumn } from "./Table";
import type { ClientFilterValue } from "./ClientFilter";

const RECENT_ROW_COUNT = 20;
const TREND_MINUTES = 60;
const MINUTE_MS = 60_000;
const PROTOCOL_TONES: Record<UpstreamProtocol, string> = {
  responses: "var(--asb-action)",
  chatCompletions: "var(--asb-claude)",
  anthropicMessages: "var(--asb-safe)",
  geminiGenerateContent: "var(--asb-warning)",
};

function formatClock(timestamp: number): string {
  const date = new Date(timestamp);
  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

interface TelemetryProps {
  metrics: GatewayMetricsStatus;
  profileNames: Map<string, string>;
  focus: ClientFilterValue;
}

/** 图表带是网关页的遥测面：累计请求带 60 分钟柱状趋势、失败请求、
 * p50/p95 耗时条与上游协议分布；「最近请求」明细常驻展示，不做折叠。
 * 全部读数只来自内存遥测样本。 */
export function GatewayTelemetry({ metrics, profileNames, focus }: TelemetryProps) {
  return (
    <>
      <GatewayStrip metrics={metrics} />
      <GatewayRecentRequests samples={metrics.samples} profileNames={profileNames} focus={focus} />
    </>
  );
}

function GatewayStrip({ metrics }: { metrics: GatewayMetricsStatus }) {
  const { t } = useI18n();
  const { totalRequests, failedRequests, samples } = metrics;
  const completed = samples
    .filter((sample) => sample.status !== null)
    .map((sample) => sample.durationMs)
    .sort((a, b) => a - b);
  const hasLatency = completed.length > 0;
  const p50 = hasLatency ? percentile(completed, 50) : null;
  const p95 = hasLatency ? percentile(completed, 95) : null;
  const buckets = buildMinuteBuckets(samples);
  const windowTotal = buckets.reduce((sum, count) => sum + count, 0);
  const max = Math.max(...buckets, 1);
  const usage = buildProtocolUsage(samples);
  return (
    <section className="asb-gateway-strip" aria-label={t("gateway.stripAriaLabel")}>
      <div className="asb-gateway-gauge is-trend">
        <p className="asb-gateway-gauge-label">{t("gateway.totalRequests")}</p>
        <p className="asb-gateway-gauge-value">{totalRequests}</p>
        <p className="asb-gateway-gauge-detail">{t("gateway.totalDetail", { count: windowTotal })}</p>
        {windowTotal > 0 && (
          <div className="asb-gateway-trend" aria-hidden="true">
            {buckets.map((count, index) => (
              <span
                key={index}
                className="asb-gateway-trend-bar"
                style={{ height: barHeightPercent(count, max) }}
              />
            ))}
          </div>
        )}
      </div>
      <div className={`asb-gateway-gauge${failedRequests > 0 ? " is-warning" : ""}`}>
        <p className="asb-gateway-gauge-label">{t("gateway.failedRequests")}</p>
        <p className="asb-gateway-gauge-value">{failedRequests}</p>
        <p className="asb-gateway-gauge-detail">{t("gateway.failedDetail")}</p>
      </div>
      <div className="asb-gateway-gauge">
        <p className="asb-gateway-gauge-label">{t("gateway.latencyTitle")}</p>
        {hasLatency && (
          <div className="asb-gateway-latency">
            <LatencyRow label={t("gateway.latencyMedian")} ms={p50} scaleMax={p95} />
            <LatencyRow label={t("gateway.latencyP95")} ms={p95} scaleMax={p95} />
          </div>
        )}
        <p className="asb-gateway-gauge-detail">{hasLatency ? t("gateway.latencyOnlyCompleted") : t("gateway.latencyNoCompleted")}</p>
      </div>
      <div className="asb-gateway-gauge is-protocol">
        <p className="asb-gateway-gauge-label">{t("gateway.protocolTitle")}</p>
        {usage.length === 0 ? (
          <p className="asb-gateway-empty" role="status">{t("gateway.noRequests")}</p>
        ) : (
          <>
            <div className="asb-gateway-protocol-bar" aria-hidden="true">
              {usage.map((entry) => (
                <span
                  key={entry.protocol}
                  style={{
                    flexGrow: entry.count,
                    backgroundColor: PROTOCOL_TONES[entry.protocol],
                  }}
                />
              ))}
            </div>
            <p className="asb-gateway-protocol-legend">
              {usage
                .map((entry) => `${protocolLabel(t, entry.protocol)} ${entry.count}`)
                .join(" · ")}
            </p>
          </>
        )}
      </div>
    </section>
  );
}

/** 柱高以窗口内最大分钟计数满高，非零柱保底可见。 */
function barHeightPercent(count: number, max: number): string {
  if (count === 0 || max === 0) return "0%";
  return `${Math.max(3, Math.round((count / max) * 100))}%`;
}

function LatencyRow({ label, ms, scaleMax }: { label: string; ms: number | null; scaleMax: number | null }) {
  const width = ms === null || scaleMax === null || scaleMax <= 0 ? 0 : Math.min(100, (ms / scaleMax) * 100);
  return (
    <div className="asb-gateway-latency-row">
      <span className="asb-gateway-latency-label">{label}</span>
      <span className="asb-gateway-latency-track" aria-hidden="true">
        <span className="asb-gateway-latency-fill" style={{ width: `${width}%` }} />
      </span>
      <span className="asb-gateway-latency-value asb-num">{ms === null ? "—" : formatDuration(ms)}</span>
    </div>
  );
}

function buildMinuteBuckets(samples: GatewaySample[]): number[] {
  const startMinute = Math.floor(Date.now() / MINUTE_MS) - (TREND_MINUTES - 1);
  const buckets = new Array<number>(TREND_MINUTES).fill(0);
  for (const sample of samples) {
    const index = Math.floor(sample.atMs / MINUTE_MS) - startMinute;
    if (index >= 0 && index < TREND_MINUTES) buckets[index] += 1;
  }
  return buckets;
}

function buildProtocolUsage(samples: GatewaySample[]): Array<{ protocol: UpstreamProtocol; count: number }> {
  const counts = new Map<UpstreamProtocol, number>();
  for (const sample of samples) {
    if (sample.upstreamProtocol !== null) {
      counts.set(sample.upstreamProtocol, (counts.get(sample.upstreamProtocol) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .map(([protocol, count]) => ({ protocol, count }))
    .sort((a, b) => b.count - a.count);
}

function percentile(sorted: number[], p: number): number {
  const index = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.min(sorted.length - 1, Math.max(0, index))];
}

function formatDuration(durationMs: number): string {
  return durationMs < 1000 ? `${durationMs} ms` : `${(durationMs / 1000).toFixed(1)} s`;
}

function GatewayRecentRequests({ samples, profileNames, focus }: {
  samples: GatewaySample[];
  profileNames: Map<string, string>;
  focus: ClientFilterValue;
}) {
  const { t } = useI18n();
  const scoped = focus === "all" ? samples : samples.filter((sample) => sample.app === focus);
  const recent = scoped.slice(-RECENT_ROW_COUNT).reverse();
  return (
    <section className="asb-gateway-recent" aria-label={t("gateway.recentTitle")}>
      <header className="asb-gateway-recent-heading">
        <h3 className="asb-section-title">{t("gateway.recentTitle")}</h3>
        <span className="asb-gateway-recent-count" role="status">
          {focus !== "all" && <>{clientFullName(focus)} · </>}
          {recent.length > 0 ? t("gateway.recentCount", { count: recent.length }) : t("gateway.noRecords")}
        </span>
      </header>
      {recent.length > 0 && (
        <Table
          ariaLabel={t("gateway.recentTableAriaLabel")}
          columns={recentColumns(t, profileNames)}
          rows={recent}
          rowKey={(sample) =>
            `${sample.atMs}:${sample.app}:${sample.profileId ?? "-"}:${sample.routeRevision ?? "-"}`
          }
          className="asb-gateway-recent-table"
        />
      )}
      <p className="asb-gateway-note">
        {t("gateway.telemetryNote")}
      </p>
    </section>
  );
}

function recentColumns(t: TFunction, profileNames: Map<string, string>): Array<TableColumn<GatewaySample>> {
  return [
    {
      key: "time",
      header: t("gateway.colTime"),
      render: (sample) => <span className="asb-gateway-row-value asb-code">{formatClock(sample.atMs)}</span>,
    },
    { key: "app", header: t("gateway.client"), cellClassName: "asb-gateway-request-app", render: (sample) => clientFullName(sample.app) },
    {
      key: "profile",
      header: t("gateway.colProvider"),
      render: (sample) =>
        sample.profileId === null
          ? t("gateway.noRouteMatch")
          : profileNames.get(sample.profileId) ?? t("gateway.deletedProvider"),
    },
    {
      key: "routeRevision",
      header: t("gateway.colRouteRevision"),
      render: (sample) =>
        sample.routeRevision === null ? (
          "—"
        ) : (
          <span className="asb-gateway-revision asb-code">{sample.routeRevision}</span>
        ),
    },
    {
      key: "path",
      header: t("gateway.colPath"),
      render: (sample) => <GatewayRequestPath sample={sample} />,
    },
    {
      key: "status",
      header: t("gateway.colStatus"),
      cellClassName: "asb-gateway-request-result",
      render: (sample) => {
        if (sample.status === null) return <span className="asb-fail-text">{t("gateway.interrupted")}</span>;
        if (sample.status === 206) return <span className="asb-fail-text">{t("gateway.partialOutput")}</span>;
        if (sample.status === 499) return <span className="asb-fail-text">{t("gateway.clientDisconnected")}</span>;
        const status = t("gateway.accountingStatus", { status: sample.status });
        if (sample.status >= 400) return <span className="asb-fail-text">{t("gateway.requestFailed", { status })}</span>;
        return <span className="asb-gateway-row-value">{t("gateway.requestSucceeded", { status })}</span>;
      },
    },
    {
      key: "duration",
      align: "end",
      header: t("gateway.colDuration"),
      render: (sample) => (
        <span className="asb-gateway-row-value">{formatDuration(sample.durationMs)}</span>
      ),
    },
  ];
}

function GatewayRequestPath({ sample }: { sample: GatewaySample }) {
  const { t } = useI18n();
  if (sample.upstreamProtocol === null) return t("gateway.noRouteMatch");
  const translated = isProtocolTranslation(sample.clientProtocol, sample.upstreamProtocol);
  return (
    <span className="asb-gateway-request-path">
      <span>{protocolLabel(t, sample.clientProtocol)}</span>
      <span aria-hidden="true">→</span>
      <span>{protocolLabel(t, sample.upstreamProtocol)}</span>
      <span className={`asb-gateway-request-mode is-${translated ? "translation" : "relay"}`}>
        {translated ? t("gateway.modeTranslation") : t("gateway.modeRelay")}
      </span>
    </span>
  );
}
