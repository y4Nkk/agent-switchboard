import { useId } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { ModelRouteGraph } from "../model-routing/ModelRouteGraph";
import { RouteInspector } from "../model-routing/RouteInspector";
import { resetRouting, routeIssue, routeRecords, type RouteDocument, type RouteInventory, type RouteRole } from "../model-routing/model";

export function ProviderRoutingPanel({ document, inventory, role, onRole, onChange, busy }: {
  document: RouteDocument; inventory: RouteInventory; role: RouteRole;
  onRole: (role: RouteRole) => void; onChange: (document: RouteDocument) => void; busy: boolean;
}) {
  const { t } = useI18n();
  const id = useId();
  const app = document.kind === "codex" ? "codex" : "claude";
  const issue = routeIssue(document, inventory);
  return <section className="asb-editor-section asb-provider-routing" aria-label={t("providerEditor.section.models")}>
    <h3 className="asb-section-title">{t("providerEditor.routingTitle")}</h3>
    <p className="asb-field-help">{t("providerEditor.routingHelp")}</p>
    <ModelRouteGraph app={app} document={document} inventory={inventory} role={role}
      onSelect={onRole} inspectorId={id} disabled={busy} />
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
