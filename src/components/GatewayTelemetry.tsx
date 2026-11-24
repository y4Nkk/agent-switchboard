import type { GatewayMetricsStatus, GatewaySample } from "../api/client";
import { useI18n, type TFunction } from "../i18n";
import { isProtocolTranslation, protocolLabel } from "../lib/protocol";
import { clientFullName } from "../lib/client-name";
import { Table, type TableColumn } from "./Table";
import { GatewayCharts } from "./GatewayCharts";
import type { ClientFilterValue } from "./ClientFilter";

const RECENT_ROW_COUNT = 20;

function formatClock(timestamp: number): string {
  const date = new Date(timestamp);
  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

interface TelemetryProps {
  metrics: GatewayMetricsStatus;
  profileNames: Map<string, string>;
  focus: ClientFilterValue;
}

/** Statistics cover the gateway snapshot; client focus scopes recent requests. */
export function GatewayTelemetry({ metrics, profileNames, focus }: TelemetryProps) {
  return (
    <>
      <GatewayCharts metrics={metrics} />
      <GatewayRecentRequests samples={metrics.samples} profileNames={profileNames} focus={focus} />
    </>
  );
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
        if (sample.status === 499) return <span className="asb-fail-text">{t("gateway.requestClientDisconnected")}</span>;
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
