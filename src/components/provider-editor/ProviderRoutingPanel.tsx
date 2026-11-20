import { useId, useRef } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Table } from "../Table";
import { RouteInspector } from "../model-routing/RouteInspector";
import { resetRouting, roleTarget, routeDescription, routeRoles, routeIssue, routeRecords, type RouteDocument, type RouteInventory, type RouteRole } from "../model-routing/model";

export function ProviderRoutingPanel({ document, inventory, role, onRole, onChange, busy }: {
  document: RouteDocument; inventory: RouteInventory; role: RouteRole;
  onRole: (role: RouteRole) => void; onChange: (document: RouteDocument) => void; busy: boolean;
}) {
  const { t } = useI18n();
  const id = useId();
  const root = useRef<HTMLElement>(null);
  const app = document.kind === "codex" ? "codex" : "claude";
  const issue = routeIssue(document, inventory);
  const select = (next: RouteRole) => {
    onRole(next);
    if (next === "main") root.current?.parentElement?.querySelector<HTMLInputElement>("[data-provider-model-entry]")?.focus();
  };
  return <section ref={root} className="asb-editor-section asb-provider-routing" aria-label={t("providerEditor.section.models")}>
    <h3 className="asb-section-title">{t("providerEditor.routingTitle")}</h3>
    <p className="asb-field-help">{t("providerEditor.routingHelp")}</p>
    <Table rows={routeRoles(document)} rowKey={(item) => item} ariaLabel={t("providerEditor.routingTitle")} columns={[
      { key: "purpose", header: t("providerSetup.purpose"), render: (item) => t(`modelRoutes.role.${item}`) },
      { key: "target", header: t("providerSetup.destination"), render: (item) => {
        const target = roleTarget(document, item, inventory);
        return target ? <span>{target.provider || t("modelRoutes.unresolved")} · {target.model
          || t(document.kind === "codex" ? "providerSetup.chooseModel" : "modelRoutes.clientDefault")}</span> : t("modelRoutes.automatic");
      } },
      { key: "mode", header: t("providerSetup.source"), render: (item) => {
        const description = routeDescription(document, item, inventory);
        const inherited = document.kind === "profile" && item === "fable" && document.draft.modelOptions?.kind === "claude"
          && !document.draft.modelOptions.routes.fable && document.draft.modelOptions.routes.opus;
        return <span>{t(inherited ? "modelRoutes.inheritOpus" : !description.target ? "modelRoutes.automatic"
          : description.mode === "provider" ? "modelRoutes.specify" : "modelRoutes.primaryModelMode")}{description.oneM ? " · 1M" : ""}</span>;
      } },
      { key: "edit", header: t("providerSetup.actions"), render: (item) => <Button variant="ghost" size="compact" disabled={busy}
        aria-label={t("modelRoutes.editNode", { role: t(`modelRoutes.role.${item}`) })} aria-pressed={role === item} aria-controls={item === "main" ? undefined : id} onClick={() => select(item)}>{t("providerSetup.edit")}</Button> },
    ]} />
    <section className="asb-model-route-inspector" id={id} aria-labelledby={`${id}-title`}>
      <h3 className="asb-section-title" id={`${id}-title`}>{t("modelRoutes.configureRole", { role: t(`modelRoutes.role.${role}`) })}</h3>
      <div className="asb-model-route-fields"><RouteInspector document={document} role={role}
        sources={routeRecords(inventory, app)} codex={inventory.codex} disabled={busy} onChange={onChange} /></div>
    </section>
    {issue && <p className="asb-warn-text" role="status">{t(issue)}</p>}
    <div className="asb-model-route-actions"><Button variant="secondary" disabled={busy}
      onClick={() => onChange(resetRouting(document))}>{t("providerEditor.resetPurposes")}</Button></div>
  </section>;
}
