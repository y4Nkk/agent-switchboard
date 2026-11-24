import { useId, useState, type ReactNode } from "react";
import type { AppKind } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { ClientLogo } from "../ClientLogo";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Tabs } from "../Tabs";
import { ModuleHeader, WorkspaceHeader } from "../WorkspaceHeader";
import { GatewayArtwork } from "./GatewayArtwork";
import { RouteCard } from "./RouteCard";
import { routeCardState, routeDocument, routeRecords, routeRoles, type RouteDocument, type RouteInventory, type RouteRole } from "./model";

interface Props {
  runtimeSummary: ReactNode;
  runtimeObservation: ReactNode;
  visible: boolean;
  inventory: RouteInventory;
  initialApp: AppKind;
  busy: boolean;
  onEdit: (app: AppKind, profileId: string, role: RouteRole) => void;
  onProviders: (app: AppKind) => void;
}

function SavedPlan({ document, app, inventory, busy, onEdit }: {
  document: RouteDocument; app: AppKind; inventory: RouteInventory; busy: boolean; onEdit: Props["onEdit"];
}) {
  const { t } = useI18n();
  const official = document.kind === "profile" && document.draft.routeMode === "official";
  return <>
    <ul className="asb-purpose-cards" data-app={app} aria-label={t("gatewayOverview.destinations")}>
      {routeRoles(document).map((role) => <RouteCard key={role} role={role} state={routeCardState(document, role, inventory)}
        fallback={t("modelRoutes.clientDefault")} disabled={busy} onOpen={() => onEdit(app, document.profileId, role)} />)}
    </ul>
    {official && <p className="asb-field-help">{t("modelRoutes.official")}</p>}
  </>;
}

/** Saved plans stay read-only here; every edit enters the provider-owned session. */
export function ModelRoutingWorkspace({
  runtimeSummary, runtimeObservation, visible, inventory, initialApp, busy, onEdit, onProviders,
}: Props) {
  const { t } = useI18n();
  const id = useId();
  const [app, setApp] = useState(initialApp);
  const [selections, setSelections] = useState<Partial<Record<AppKind, string>>>({});
  const sources = routeRecords(inventory, app);
  const activeId = inventory.activeIds[app] ?? null;
  const activeSource = sources.find(({ record }) => record.profile.id === activeId);
  const requestedId = selections[app] ?? activeId;
  const selected = sources.find(({ record }) => record.profile.id === requestedId);
  const document = selected ? routeDocument(selected) : null;
  const selectedId = selected?.record.profile.id ?? null;
  return <section className="asb-panel asb-model-routing" aria-label={t("nav.page.gateway")}>
    <WorkspaceHeader title={t("nav.page.gateway")} />
    <section className="asb-route-overview-runtime" aria-label={t("gatewayOverview.runtime")}>
      <div className="asb-route-overview-listener">
        <div className="asb-route-overview-artwork">
          <GatewayArtwork active={visible} />
        </div>
        {runtimeSummary}
      </div>
      {runtimeObservation}
    </section>
    <section className="asb-route-overview-plan" aria-labelledby={`${id}-title`}>
      <ModuleHeader id={`${id}-title`} title={t("gatewayOverview.savedPlan")} primary={
        <Tabs value={app} onChange={setApp} scope={id} label={t("modelRoutes.client")}
          tabs={(["codex", "claude"] as const).map((value) => ({ value, label: <><ClientLogo app={value} className="asb-model-route-logo" />{clientName(value)}</>, controls: `${id}-panel`, disabled: busy }))} />
      } primaryActions={<Button variant="secondary" disabled={busy} onClick={() => onProviders(app)}>{t("modelRoutes.manageProviders")}</Button>} />
      <div role="tabpanel" id={`${id}-panel`} aria-labelledby={`${id}-${app}-tab`} className="asb-route-overview-panel">
        <div className="asb-route-overview-selection">
          <label className="asb-field"><span>{t("gatewayOverview.viewing")}</span>
            <Select value={selectedId} placeholder={t("modelRoutes.chooseProvider")} ariaLabel={t("gatewayOverview.viewing")}
              disabled={busy || sources.length === 0} onChange={(profileId) => setSelections((current) => ({ ...current, [app]: profileId }))}
              options={sources.map(({ record }) => ({ value: record.profile.id, label: record.profile.name }))} />
          </label>
          <Button variant="secondary" disabled={busy || !activeSource || selectedId === activeId}
            onClick={() => activeId && setSelections((current) => ({ ...current, [app]: activeId }))}>{t("gatewayOverview.viewCurrent")}</Button>
        </div>
        <div className="asb-route-overview-identity">
          <FactList inline rows={[{ label: t("gatewayOverview.active"), value: activeSource?.record.profile.name ?? t("modelRoutes.unmatched") }]} />
          {document && <p className="asb-route-overview-saved">{t(selectedId === activeId ? "gatewayOverview.activePlan" : "gatewayOverview.inactivePlan")}</p>}
        </div>
        {document ? <SavedPlan document={document} app={app} inventory={inventory} busy={busy} onEdit={onEdit} />
          : <div className="asb-route-overview-empty"><p>{t(requestedId ? "gatewayOverview.missingPlan" : "gatewayOverview.empty")}</p>
            <Button variant="secondary" disabled={busy} onClick={() => onProviders(app)}>{t("modelRoutes.manageProviders")}</Button></div>}
        <p className="asb-route-overview-footnote">{t("gatewayOverview.savedOnly")}</p>
      </div>
    </section>
  </section>;
}
