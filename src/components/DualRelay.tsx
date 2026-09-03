import type { AppKind, RouteState } from "../api/client";
import { ClientLogo } from "./ClientLogo";
import { MatrixStarlightCanvas } from "./experience/MatrixStarlightCanvas";

interface Props {
  routes: { codex: RouteState | null; claude: RouteState | null };
  providerNames: Partial<Record<AppKind, string>>;
}

function hostLabel(url: string | null): string | null {
  if (!url) return null;
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/** Real routing facts only; the product has no traffic or usage data source. */
function accessLabel(route: RouteState | null): string {
  if (!route) return "—";
  if (route.routeMode === "official") return "官方登录";
  return route.apiKey ? `$${route.apiKey}` : "自定义";
}

function RouteCard({
  app,
  route,
  providerName,
}: {
  app: AppKind;
  route: RouteState | null;
  providerName: string;
}) {
  const client = app === "codex" ? "Codex" : "Claude";
  const host = hostLabel(route?.baseUrl ?? null);
  const on = route !== null;

  return (
    <article
      className={`asb-route-card${on ? " is-on" : ""}`}
      data-app={app}
      aria-label={`${client} 当前配置`}
    >
      <MatrixStarlightCanvas variant={on ? "route-active" : "route-idle"} />
      <div className="asb-route-card-body">
        <div>
          <div className="asb-route-ident">
            <ClientLogo app={app} className="asb-route-logo" />
            <span className="asb-route-client">{client}</span>
          </div>
          <h3 className="asb-route-provider">{providerName}</h3>
        </div>
        <dl className="asb-route-values">
          <div>
            <dt className="asb-route-key">模型</dt>
            <dd className="asb-route-value">{route?.model ?? "—"}</dd>
          </div>
          <div>
            <dt className="asb-route-key">服务地址</dt>
            <dd className="asb-route-value">{host ?? "—"}</dd>
          </div>
          <div>
            <dt className="asb-route-key">接入方式</dt>
            <dd className="asb-route-value">{accessLabel(route)}</dd>
          </div>
        </dl>
      </div>
    </article>
  );
}

/** The Dual Relay: the one intentionally expressive control (DESIGN.md §4). */
export function DualRelay({ routes, providerNames }: Props) {
  return (
    <section className="asb-routebar asb-surface-rail" aria-label="当前启用配置">
      <h2 className="asb-panel-title">当前启用配置</h2>
      <div className="asb-route-cards">
        <RouteCard
          app="codex"
          route={routes.codex}
          providerName={providerNames.codex ?? "未加载"}
        />
        <RouteCard
          app="claude"
          route={routes.claude}
          providerName={providerNames.claude ?? "未加载"}
        />
      </div>
    </section>
  );
}
