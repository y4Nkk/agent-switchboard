import type { ProviderProfile, RouteState } from "../api/client";

interface Props {
  routes: { codex: RouteState | null; claude: RouteState | null };
  selectedProfile: ProviderProfile | null;
  canSwitch: boolean;
  busy: boolean;
  onPreview: () => void;
  onSwitch: () => void;
}

function laneState(route: RouteState | null): string {
  if (!route) return "未加载";
  const provider =
    route.providerName ?? (route.routeMode === "custom" ? "自定义服务" : "官方登录");
  return route.model ? `${provider} · ${route.model}` : provider;
}

/** The Dual Relay: the one intentionally expressive control (DESIGN.md §4). */
export function DualRelay({ routes, selectedProfile, canSwitch, busy, onPreview, onSwitch }: Props) {
  return (
    <section className="asb-routebar asb-glass" aria-label="当前路由">
      <div className="asb-lanes">
        <div className="asb-lane asb-lane-codex">
          <span className="asb-lane-name">Codex</span>
          <span className="asb-lane-rail" aria-hidden="true" />
          <span
            className={`asb-lane-node${routes.codex ? " is-active" : ""}`}
            aria-hidden="true"
          />
          <span className="asb-lane-value">{laneState(routes.codex)}</span>
        </div>
        <div className="asb-lane asb-lane-claude">
          <span className="asb-lane-name">Claude</span>
          <span className="asb-lane-rail" aria-hidden="true" />
          <span
            className={`asb-lane-node asb-node-claude${routes.claude ? " is-active" : ""}`}
            aria-hidden="true"
          />
          <span className="asb-lane-value">{laneState(routes.claude)}</span>
        </div>
      </div>
      <div className="asb-routebar-actions">
        <button type="button" className="asb-btn-secondary" disabled={!selectedProfile || busy} onClick={onPreview}>
          查看变更
        </button>
        <button type="button" className="asb-btn-primary" disabled={!canSwitch || busy} onClick={onSwitch}>
          安全切换
        </button>
      </div>
    </section>
  );
}
