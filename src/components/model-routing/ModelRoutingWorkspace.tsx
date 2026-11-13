import { useId, useState, type ReactNode } from "react";
import type { AppKind } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import routingHub from "../../assets/routing-hub.png";
import { Button } from "../Button";
import { ClientLogo } from "../ClientLogo";
import { FactList } from "../FactList";
import { ChevronRightIcon } from "../icons";
import { Select } from "../Select";
import { Tabs } from "../Tabs";
import { ModuleHeader, WorkspaceHeader } from "../WorkspaceHeader";
import { routeDescription, routeDocument, routeIssue, routeRecords, routeRoles,
  type RouteDocument, type RouteInventory, type RouteRole } from "./model";

interface Props {
  runtimeSummary: ReactNode;
  inventory: RouteInventory;
  initialApp: AppKind;
  busy: boolean;
  onEdit: (app: AppKind, profileId: string, role: RouteRole) => void;
  onProviders: (app: AppKind) => void;
}

function SavedPurpose({ document, inventory, role, disabled, onEdit }: {
  document: RouteDocument; inventory: RouteInventory; role: RouteRole; disabled: boolean; onEdit: () => void;
}) {
  const { t } = useI18n();
  const { target, mode, oneM } = routeDescription(document, role, inventory);
  const source = role === "main" ? "gatewayOverview.mainConnection"
    : !target ? "modelRoutes.automatic" : mode === "provider" ? "gatewayOverview.fixedProvider" : "gatewayOverview.primaryConnection";
  const provider = target ? target.provider ?? t("modelRoutes.unresolved") : t("modelRoutes.automatic");
  const model = target?.model || t("modelRoutes.clientDefault");
  return <li className="asb-route-overview-purpose">
    <Button variant="ghost" className="asb-route-overview-edit" disabled={disabled} onClick={onEdit}
      aria-label={t("gatewayOverview.editPurpose", { role: t(`modelRoutes.role.${role}`), provider, model })}>
      <span className="asb-route-overview-role">{t(`modelRoutes.role.${role}`)}</span>
      <span className="asb-route-overview-target">
        <span className="asb-route-overview-provider">{provider}</span>
        <span className="asb-route-overview-model asb-code">{model}{oneM && <span className="asb-route-overview-context">1M</span>}</span>
      </span>
      <span className="asb-route-overview-mode">{t(source)}</span>
      <span className="asb-route-overview-arrow" aria-hidden="true"><ChevronRightIcon /></span>
    </Button>
  </li>;
}

function SavedPlan({ document, app, inventory, busy, onEdit }: {
  document: RouteDocument; app: AppKind; inventory: RouteInventory; busy: boolean; onEdit: Props["onEdit"];
}) {
  const { t } = useI18n();
  const issue = routeIssue(document, inventory);
  const official = document.kind === "profile" && document.draft.routeMode === "official";
  return <>
    <div className="asb-route-overview-columns" aria-hidden="true">
      <span>{t("modelRoutes.purpose")}</span><span>{t("gatewayOverview.destination")}</span><span>{t("gatewayOverview.rule")}</span>
    </div>
    <ul className="asb-route-overview-list" aria-label={t("gatewayOverview.destinations")}>
      {routeRoles(document).map((role) => <SavedPurpose key={role} document={document} inventory={inventory}
        role={role} disabled={busy} onEdit={() => onEdit(app, document.profileId, role)} />)}
    </ul>
    {official && <p className="asb-field-help">{t("modelRoutes.official")}</p>}
    {issue && <p className="asb-warn-text" role="status">{t(issue)}</p>}
  </>;
}

/** Saved plans stay read-only here; every edit enters the provider-owned session. */
export function ModelRoutingWorkspace({ runtimeSummary, inventory, initialApp, busy, onEdit, onProviders }: Props) {
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
    <div className="asb-route-overview-hero">
      <div className="asb-route-overview-intro">
        <WorkspaceHeader title={t("nav.page.gateway")} />
        <p className="asb-route-overview-description">{t("gatewayOverview.description")}</p>
        <p className="asb-route-overview-caption">{t("gatewayOverview.editHelp")}</p>
      </div>
      <img className="asb-route-overview-art" src={routingHub} alt="" aria-hidden="true" draggable={false} />
    </div>
    <section className="asb-route-overview-runtime" aria-label={t("gatewayOverview.listener")}>{runtimeSummary}</section>
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
