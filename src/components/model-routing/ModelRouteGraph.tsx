import { Fragment, useRef } from "react";
import type { AppKind, CodexProviderRecord } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { ClientLogo } from "../ClientLogo";
import { GatewayDeviceArt } from "../GatewayDeviceArt";
import { RouteConnections } from "./RouteConnections";
import { roleTarget, routeRoles, type RouteDocument, type RouteRole } from "./model";

export function ModelRouteGraph({ app, document, codex, role, onSelect, inspectorId, disabled }: {
  app: AppKind; document: RouteDocument; codex: CodexProviderRecord[];
  role: RouteRole; onSelect: (role: RouteRole) => void; inspectorId: string; disabled: boolean;
}) {
  const { t } = useI18n();
  const scene = useRef<HTMLDivElement>(null);
  const roles = routeRoles(document);
  return <section className="asb-model-route-graph" aria-label={t("modelRoutes.graphAria")}>
    <div className="asb-model-route-scene" ref={scene}>
      <RouteConnections scene={scene} roles={roles} selected={role} />
      <div className="asb-model-route-client" data-route-node="client" style={{ gridRow: `1 / span ${roles.length}` }}>
        <GatewayDeviceArt kind="client" />
        <strong><ClientLogo app={app} className="asb-model-route-logo" />{clientName(app)}</strong>
        <span>{t("modelRoutes.clientNode")}</span>
      </div>
      {roles.map((item, index) => {
        const target = roleTarget(document, item, codex);
        const choosing = document.kind === "codex" && item === "subagent" && document.draft.subagentRoute?.profileId === "";
        return <Fragment key={item}>
          <div className="asb-model-route-purpose" data-route-node={`purpose-${item}`} style={{ gridRow: index + 1 }}>
            <span>{t(`modelRoutes.role.${item}`)}</span>
          </div>
          <div className="asb-model-route-destination" data-route-node={`target-${item}`} style={{ gridRow: index + 1 }}>
            <Button variant="ghost" className="asb-model-route-target" disabled={disabled}
              aria-pressed={role === item} aria-controls={inspectorId}
              aria-label={t("modelRoutes.editNode", { role: t(`modelRoutes.role.${item}`) })}
              onClick={() => onSelect(item)}>
              <span className="asb-model-route-target-content" data-selected={role === item || undefined}>
                <span className="asb-model-route-provider">{target
                  ? target.provider ?? t(choosing ? "modelRoutes.chooseProvider" : "modelRoutes.unresolved") : t("modelRoutes.automatic")}</span>
                <span className="asb-model-route-model">{target?.model || t(item === "subagent" && target ? "modelRoutes.chooseTarget" : "modelRoutes.clientDefault")}</span>
                <span className="asb-model-route-edit">{t("modelRoutes.edit")}</span>
              </span>
            </Button>
          </div>
        </Fragment>;
      })}
    </div>
    <p className="asb-field-help">{t("modelRoutes.graphHelp")}</p>
  </section>;
}
