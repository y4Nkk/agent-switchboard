import { useMemo, useState } from "react";
import type { AppKind, GatewayRouteStatus, GatewaySample, GatewayStatus } from "../api/client";
import { useI18n } from "../i18n";
import { clientFullName } from "../lib/client-name";
import { NATIVE_PROTOCOL, isProtocolTranslation, protocolLabel } from "../lib/protocol";
import { Button } from "./Button";
import { ClientLogo } from "./ClientLogo";
import { ClientFilter, type ClientFilterValue } from "./ClientFilter";
import { GatewayDeviceArt } from "./GatewayDeviceArt";
import { GatewayTopologyWires } from "./GatewayTopologyWires";
import { ConfirmGatewayRecoveryDiscard } from "./GatewayPagePrimitives";

const APP_ORDER: AppKind[] = ["codex", "claude"];
const TRAFFIC_WINDOW_MS = 3_600_000;
const FLOW_WINDOW_MS = 10_000;

interface Props {
  status: GatewayStatus;
  failed: boolean;
  snapshotCurrent: boolean;
  profileNames: Map<string, string>;
  focus: ClientFilterValue;
  onFocusChange: (value: ClientFilterValue) => void;
}

export function GatewayTopology({ status, failed, snapshotCurrent, profileNames, focus, onFocusChange }: Props) {
  const { t } = useI18n();
  const [scene, setScene] = useState<HTMLDivElement | null>(null);
  const { routes } = status;
  const rows = Math.max(APP_ORDER.length, routes.length);
  const canShowFlow = snapshotCurrent && status.status === "running" && status.listeningPort !== null;
  const traffic = useTrafficCounts(status.metrics.samples, routes, canShowFlow);
  const toggleFocus = (app: AppKind) => onFocusChange(focus === app ? "all" : app);
  return (
    <section className={`asb-gateway-map${failed ? " is-failure" : ""}`} aria-label={t("gateway.mapAriaLabel")}>
      <header className="asb-gateway-map-heading">
        <h4 className="asb-group-title">{t("gateway.topologyTitle")}</h4>
        <ClientFilter value={focus} onChange={onFocusChange} label={t("gateway.focusAriaLabel")} showLogos />
      </header>
      <div ref={setScene} className="asb-gateway-map-canvas" style={{ gridTemplateRows: `repeat(${rows}, auto)` }}>
        <GatewayTopologyWires scene={scene} routes={routes} focus={focus} failed={failed}
          flowByApp={traffic.flowByApp} flowByRoute={traffic.flowByRoute} />
        <GatewayClientNodes rows={[1, rows]} routes={routes} counts={traffic.byApp} focus={focus} onFocus={toggleFocus} />
        <div data-gateway-node="hub" className="asb-gateway-node is-hub" style={{ gridRow: `1 / span ${rows}` }}>
          <GatewayDeviceArt kind="hub" connected={routes.length > 0} /><strong>{t("gateway.title")}</strong>
          <span className="asb-gateway-node-caption">{status.baseUrl ?? t("gateway.addressNotListening")}</span>
        </div>
        <GatewayProviderNodes routes={routes} profileNames={profileNames} counts={traffic.byRoute} focus={focus} onFocus={toggleFocus} />
        {routes.length === 0 && (
          <div className="asb-gateway-node is-empty" style={{ gridRow: `1 / span ${rows}` }} role="status">
            <GatewayDeviceArt kind="provider" connected={false} />
            <strong>{t("gateway.emptyRoutesTitle")}</strong>
            <span>{t("gateway.emptyRoutesCopy")}</span>
          </div>
        )}
      </div>
      <div className="asb-gateway-map-footer">
        <div className="asb-gateway-legend" aria-label={t("gateway.legendAriaLabel")}>
          <span><i className="is-translation" aria-hidden="true" />{t("gateway.modeTranslation")}</span>
          <span><i aria-hidden="true" />{t("gateway.modeRelay")}</span>
        </div>
        <p className="asb-gateway-focus-hint">{t("gateway.focusHint")}</p>
      </div>
    </section>
  );
}

function routeKey(route: Pick<GatewayRouteStatus, "app" | "profileId">): string {
  return `${route.app}:${route.profileId}`;
}

function useTrafficCounts(samples: GatewaySample[], routes: GatewayRouteStatus[], canShowFlow: boolean) {
  return useMemo(() => {
    const now = Date.now();
    const sinceMs = now - TRAFFIC_WINDOW_MS;
    const currentRoutes = new Map(routes.map((route) => [routeKey(route), route.upstreamProtocol]));
    const byRoute = new Map<string, number>();
    const byApp = new Map<AppKind, number>();
    const flowByRoute = new Map<string, number>();
    const flowByApp = new Map<AppKind, number>();
    for (const sample of samples) {
      if (sample.atMs < sinceMs || sample.profileId === null) continue;
      const key = routeKey({ app: sample.app, profileId: sample.profileId });
      const protocol = currentRoutes.get(key);
      if (protocol === undefined) continue;
      byRoute.set(key, (byRoute.get(key) ?? 0) + 1);
      byApp.set(sample.app, (byApp.get(sample.app) ?? 0) + 1);
      if (canShowFlow && sample.upstreamProtocol === protocol
        && sample.atMs >= now - FLOW_WINDOW_MS && sample.atMs <= now) {
        flowByRoute.set(key, Math.max(sample.atMs, flowByRoute.get(key) ?? 0));
        flowByApp.set(sample.app, Math.max(sample.atMs, flowByApp.get(sample.app) ?? 0));
      }
    }
    return { byRoute, byApp, flowByRoute, flowByApp };
  }, [samples, routes, canShowFlow]);
}

interface FocusProps {
  focus: ClientFilterValue;
  onFocus: (app: AppKind) => void;
}

function GatewayClientNodes({ rows, routes, counts, focus, onFocus }: FocusProps & {
  rows: number[];
  routes: GatewayRouteStatus[];
  counts: Map<AppKind, number>;
}) {
  const { t } = useI18n();
  return APP_ORDER.map((app, index) => {
    const routeCount = routes.filter((route) => route.app === app).length;
    return (
      <div className={`asb-gateway-connection is-client${focus !== "all" && focus !== app ? " is-muted" : ""}`}
        key={app} data-client={app} style={{ gridRow: rows[index] }}>
        <Button variant="unstyled" className="asb-gateway-node is-client" aria-pressed={focus === app}
          data-gateway-node={`client:${app}`}
          aria-label={t("gateway.focusClient", { app: clientFullName(app) })} onClick={() => onFocus(app)}>
          <span className="asb-gateway-node-identity">
            <ClientLogo app={app} className="asb-gateway-node-logo" />
            <span className="asb-gateway-node-caption">{t("gateway.client")}</span>
          </span>
          <GatewayDeviceArt kind="client" connected={routeCount > 0} />
          <strong>{clientFullName(app)}</strong>
          <span className="asb-gateway-route-protocol">
            {t("gateway.requestFormat", { format: protocolLabel(t, NATIVE_PROTOCOL[app]) })}
          </span>
          <span className="asb-gateway-node-footer">
            <span className={`asb-gateway-node-connection${routeCount === 0 ? " is-disconnected" : ""}`}>
              {routeCount > 0 ? t("gateway.clientRoutes", { count: routeCount }) : t("gateway.clientDisconnected")}
            </span>
            <GatewayTraffic count={counts.get(app) ?? 0} />
          </span>
        </Button>
      </div>
    );
  });
}

function GatewayProviderNodes({ routes, profileNames, counts, focus, onFocus }: FocusProps & {
  routes: GatewayRouteStatus[];
  profileNames: Map<string, string>;
  counts: Map<string, number>;
}) {
  const { t } = useI18n();
  return routes.map((route, index) => {
    const translated = isProtocolTranslation(NATIVE_PROTOCOL[route.app], route.upstreamProtocol);
    const name = profileNames.get(route.profileId) ?? t("gateway.deletedProvider");
    return (
      <div className={`asb-gateway-connection is-provider${focus !== "all" && focus !== route.app ? " is-muted" : ""}`}
        key={routeKey(route)} data-client={route.app} style={{ gridRow: index + 1 }}>
        <Button variant="unstyled" className="asb-gateway-node is-provider" aria-pressed={focus === route.app}
          data-gateway-node={routeKey(route)}
          aria-label={t("gateway.focusProvider", { name, app: clientFullName(route.app) })}
          onClick={() => onFocus(route.app)}>
          <span className="asb-gateway-node-identity">
            <ClientLogo app={route.app} className="asb-gateway-node-logo" />
            <span className="asb-gateway-node-caption">{t("gateway.providerCaption", { app: clientFullName(route.app) })}</span>
          </span>
          <GatewayDeviceArt kind="provider" />
          <strong>{name}</strong>
          <span className="asb-gateway-route-protocol">
            {t("gateway.requestFormatPair", {
              from: protocolLabel(t, NATIVE_PROTOCOL[route.app]), to: protocolLabel(t, route.upstreamProtocol),
            })}
          </span>
          <span className="asb-gateway-node-footer">
            <span className={`asb-gateway-mode${translated ? " is-translation" : ""}`}>
              {translated ? t("gateway.modeTranslation") : t("gateway.modeRelay")}
            </span>
            <GatewayTraffic count={counts.get(routeKey(route)) ?? 0} />
          </span>
        </Button>
      </div>
    );
  });
}

function GatewayTraffic({ count }: { count: number }) {
  const { t } = useI18n();
  if (count === 0) return null;
  return <span className="asb-gateway-traffic" aria-label={t("gateway.trafficBadge", { count })}>
    {t("gateway.trafficCount", { count })}
  </span>;
}

export function GatewayGuidance({ status, onDiscardRecovery, busy }: { status: GatewayStatus; onDiscardRecovery: () => Promise<void>; busy: boolean }) {
  const { t } = useI18n();
  if (status.status !== "needsRepair" && !status.blockedRecovery) return null;
  return (
    <div className="asb-gateway-guidance">
      {status.status === "needsRepair" && (
        <div className="asb-gateway-guidance-item" role="alert">
          <p className="asb-gateway-guidance-title">{t("gateway.repairTitle")}</p>
          {status.listeningPort === null && <p className="asb-gateway-guidance-copy">{t("gateway.repairRetryCopy")}</p>}
          <p className="asb-gateway-guidance-copy">{t("gateway.repairReapplyCopy")}</p>
          {status.repairReason && (
            <p className="asb-gateway-guidance-copy">{t("gateway.failureDetail", { reason: status.repairReason })}</p>
          )}
        </div>
      )}
      {status.blockedRecovery && (
        <div className="asb-gateway-guidance-item" role="alert">
          <p className="asb-gateway-guidance-title">{t("gateway.recoveryBlockedTitle", {
            from: status.blockedRecovery.fromPort, to: status.blockedRecovery.toPort,
          })}</p>
          <p className="asb-gateway-guidance-copy">{t("gateway.recoveryBlockedCopy")}</p>
          <p className="asb-gateway-guidance-copy">{t("gateway.failureDetail", { reason: status.blockedRecovery.reason })}</p>
          <ConfirmGatewayRecoveryDiscard onConfirm={onDiscardRecovery} busy={busy} />
        </div>
      )}
    </div>
  );
}
