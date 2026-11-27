import { useState } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Select } from "../Select";
import { SettingsGroup } from "../SettingsLayout";
import { Switch } from "../Switch";
import { ToggleChip } from "../ToggleChip";
import { RouteCard } from "../model-routing/RouteCard";
import { clearRoleRoute, resetRouting, roleRoute, roleSource, roleSources, routeCardState, routeRoles, setRoleModel,
  setRoleOneM, setRoleSource, sourceKey, sourceModels, sourceName, type RouteDocument, type RouteInventory,
  type RouteRole, type RouteSource } from "../model-routing/model";

interface Props {
  document: RouteDocument;
  inventory: RouteInventory;
  onChange: (document: RouteDocument) => void;
  busy: boolean;
}

function EditorPurpose({ document, inventory, role, busy, onChange }: Props & { role: RouteRole }) {
  const { t } = useI18n();
  const state = routeCardState(document, role, inventory);
  const roleLabel = t(`modelRoutes.role.${role}`);
  const route = roleRoute(document, role);
  const [staged, setStaged] = useState<RouteSource | null>(null);
  const offered = roleSources(document, inventory);
  const current = roleSource(document, role);
  // A target that stopped being eligible stays listed so the purpose can be moved off it.
  const options = [...(current && !offered.some((item) => sourceKey(item) === sourceKey(current)) ? [current] : []), ...offered];
  const stagedKey = staged ? sourceKey(staged) : null;
  const source = current ?? options.find((item) => sourceKey(item) === stagedKey) ?? options[0] ?? null;
  const models = source && offered.some((item) => sourceKey(item) === sourceKey(source)) ? sourceModels(document, source, inventory) : [];
  const oneM = role === "main" ? state.oneM : document.kind === "profile" && route && "oneM" in route ? route.oneM : false;
  const context = document.kind === "profile" && <span className="asb-purpose-card-option">
    <span aria-hidden="true">1M</span>
    <Switch label={`${roleLabel} · ${t("modelRoutes.oneM")}`} checked={oneM} disabled={busy || !(role === "main" ? state.model : route?.model)}
      onChange={(next) => onChange(setRoleOneM(document, role, next))} />
  </span>;
  if (role === "main") return <RouteCard role={role} state={state}
    fallback={t(document.kind === "codex" ? "providerSetup.chooseModel" : "modelRoutes.clientDefault")}>
    <p className="asb-field-help">{t("modelRoutes.mainOwner", { name: document.draft.name })}</p>
    {context}
  </RouteCard>;
  const pick = (key: string) => {
    const next = options.find((item) => sourceKey(item) === key);
    if (!next) return;
    // An automatic purpose has nothing to move yet: the choice only stages which models the lane offers.
    if (current) onChange(setRoleSource(document, role, next, inventory)); else setStaged(next);
  };
  const label = (item: RouteSource) => item === "primary" ? t("modelRoutes.primaryModelMode")
    : item.profileId === document.profileId ? t("providerEditor.selfTarget", { name: document.draft.name })
    : sourceName(document, item, inventory) ?? t("modelRoutes.unresolved");
  return <RouteCard role={role} state={state} fallback={t("modelRoutes.chooseTarget")}>
    <div className="asb-purpose-card-controls">
      {options.length > 1 && <label className="asb-purpose-card-source"><span>{t("providerSetup.source")}</span>
        <Select value={source ? sourceKey(source) : null} ariaLabel={`${roleLabel} · ${t("providerSetup.source")}`} disabled={busy}
          options={options.map((item) => ({ value: sourceKey(item), label: label(item) }))} onChange={pick} /></label>}
      <div className="asb-chip-row" role="group" aria-label={t("modelRoutes.editNode", { role: roleLabel })}>
        <ToggleChip pressed={!route} label={t(state.inherited ? "modelRoutes.inheritOpus" : "modelRoutes.automatic")} disabled={busy}
          onClick={() => { if (route) onChange(clearRoleRoute(document, role)); }} />
        {models.map((model) => <ToggleChip key={model} pressed={route?.model === model} label={model} disabled={busy}
          onClick={() => { if (source && route?.model !== model) onChange(setRoleModel(document, role, source, model)); }} />)}
      </div>
      {!state.issue && !models.length && <p className="asb-field-help">{t("modelRoutes.chooseProviderFirst")}</p>}
      {context}
    </div>
  </RouteCard>;
}

export function ProviderRoutingPanel({ document, inventory, onChange, busy }: Props) {
  const { t } = useI18n();
  const roles = routeRoles(document);
  const relayed = roles.some((role) => {
    const route = roleRoute(document, role);
    return route !== null && (!("mode" in route) || route.mode === "provider");
  });
  return <SettingsGroup title={t("providerEditor.routingTitle")} surface="plain">
    <ul className="asb-purpose-cards" data-app={document.kind === "codex" ? "codex" : "claude"}
      aria-label={t("providerEditor.routingTitle")}>
      {roles.map((role) => <EditorPurpose key={role} document={document} inventory={inventory} role={role}
        busy={busy} onChange={onChange} />)}
    </ul>
    <div className="asb-model-route-actions">
      {relayed && <p className="asb-field-help">{t("modelRoutes.subagentHelp")}</p>}
      <Button variant="secondary" disabled={busy}
        onClick={() => onChange(resetRouting(document))}>{t("providerEditor.resetPurposes")}</Button></div>
  </SettingsGroup>;
}
