import { useId, type ReactNode } from "react";
import type { AppKind } from "../../api/client";
import { useI18n } from "../../i18n";
import { errorText } from "../../i18n/errors";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { ClientLogo } from "../ClientLogo";
import { ConfirmSheet } from "../ConfirmSheet";
import { RouteSavePreview } from "./RouteSavePreview";
import { Select } from "../Select";
import { Tabs } from "../Tabs";
import { WorkspaceHeader } from "../WorkspaceHeader";
import { ModelRouteGraph } from "./ModelRouteGraph";
import { RouteInspector } from "./RouteInspector";
import { resetRouting, routeIssue, type ModelRouteRequest, type RouteInventory } from "./model";
import { useRouteApply } from "./useRouteApply";
import { useRouteDraft } from "./useRouteDraft";

interface Props {
  runtimeSummary: ReactNode;
  inventory: RouteInventory;
  initialApp: AppKind;
  request: ModelRouteRequest | null;
  active: boolean;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onApplied: () => Promise<void>;
  onProviders: (app: AppKind) => void;
}

function RouteConfirmation({ operation, onDocument, busy }: {
  operation: ReturnType<typeof useRouteApply>;
  onDocument: ReturnType<typeof useRouteDraft>["setDocument"];
  busy: boolean;
}) {
  const { t } = useI18n();
  if (!operation.pending) return null;
  return <ConfirmSheet title={t("modelRoutes.confirmTitle")} confirmLabel={t(operation.pending.kind === "saveAndApply" ? "modelRoutes.apply" : "modelRoutes.saveOnly")}
    busy={busy} onCancel={() => void operation.cancel()} onConfirm={() => void operation.confirm(onDocument)}>
    <RouteSavePreview pending={operation.pending} />
    {operation.error != null && <p className="asb-fail-text" role="alert">{errorText(operation.error, t)}</p>}
  </ConfirmSheet>;
}

function RouteEditor({ editor, operation, inventory, disabled, inspectorId }: {
  editor: ReturnType<typeof useRouteDraft>; operation: ReturnType<typeof useRouteApply>;
  inventory: RouteInventory; disabled: boolean; inspectorId: string;
}) {
  const { t } = useI18n();
  const document = editor.document;
  if (!document) return <div className="asb-model-route-empty">
    <p>{t("modelRoutes.empty")}</p>
    <Select value={null} placeholder={t("modelRoutes.chooseProvider")} ariaLabel={t("modelRoutes.provider")}
      disabled={disabled || editor.sources.length === 0} onChange={editor.selectProfile}
      options={editor.sources.map(({ record }) => ({ value: record.profile.id, label: record.profile.name }))} />
  </div>;
  const issue = routeIssue(document, inventory);
  return <>
    <ModelRouteGraph app={editor.app} document={document} inventory={inventory} role={editor.role}
      onSelect={editor.setRole} inspectorId={inspectorId} disabled={disabled} />
    <section className="asb-model-route-inspector" id={inspectorId} aria-labelledby={`${inspectorId}-title`}>
      <h3 className="asb-section-title" id={`${inspectorId}-title`}>{t("modelRoutes.configureRole", { role: t(`modelRoutes.role.${editor.role}`) })}</h3>
      <div className="asb-model-route-fields">
        <RouteInspector document={document} role={editor.role} sources={editor.sources} codex={inventory.codex}
          disabled={disabled || editor.stale} onChange={editor.setDocument} />
      </div>
    </section>
    {issue && <p className="asb-warn-text" role="status">{t(issue)}</p>}
    <div className="asb-model-route-actions">
      <Button variant="secondary" disabled={disabled || editor.stale}
        onClick={() => editor.setDocument(resetRouting(document))}>{t("modelRoutes.reset")}</Button>
      <Button variant="primary" disabled={disabled || editor.stale || !!issue || !editor.dirty}
        onClick={() => void operation.prepare(document)}>{t("modelRoutes.review")}</Button>
    </div>
  </>;
}

export function ModelRoutingWorkspace(props: Props) {
  const { t } = useI18n();
  const id = useId();
  const operation = useRouteApply(props);
  const disabled = props.busy || !!operation.pending;
  const editor = useRouteDraft({ ...props, busy: disabled });
  const activeId = props.inventory.activeIds[editor.app];
  const activeRecord = editor.sources.find(({ record }) => record.profile.id === activeId);
  return <section className="asb-panel asb-model-routing" aria-label={t("nav.page.gateway")}>
    <WorkspaceHeader title={t("nav.page.gateway")} primary={
      <Tabs value={editor.app} onChange={editor.selectApp} scope={id} label={t("modelRoutes.client")}
        tabs={(["codex", "claude"] as const).map((app) => ({ value: app,
          label: <><ClientLogo app={app} className="asb-model-route-logo" />{clientName(app)}</>, controls: `${id}-panel`, disabled }))} />
    } primaryActions={<Button variant="secondary" disabled={disabled}
      onClick={() => props.onProviders(editor.app)}>{t("modelRoutes.manageProviders")}</Button>} />
    {props.runtimeSummary}
    <div role="tabpanel" id={`${id}-panel`} aria-labelledby={`${id}-${editor.app}-tab`} className="asb-model-route-panel">
      <p className="asb-field-help">{t("modelRoutes.help")}</p>
      <p className="asb-model-route-state" role="status">{t("modelRoutes.current", { name: activeRecord?.record.profile.name ?? t("modelRoutes.unmatched") })}
        {editor.document && <span>{t(editor.dirty ? "modelRoutes.pending" : editor.selectedId !== activeId ? "modelRoutes.savedInactive" : "modelRoutes.saved")}</span>}
      </p>
      <div className="asb-model-route-owner">
        <label className="asb-field"><span>{t("modelRoutes.editing")}</span>
          <Select value={editor.selectedId} placeholder={t("modelRoutes.chooseProvider")} ariaLabel={t("modelRoutes.editing")}
            disabled={disabled} onChange={editor.selectProfile}
            options={editor.sources.map(({ record }) => ({ value: record.profile.id, label: record.profile.name }))} />
        </label>
        <Button variant="secondary" disabled={disabled || !activeId || editor.selectedId === activeId}
          onClick={() => activeId && editor.selectProfile(activeId)}>{t("modelRoutes.editCurrent")}</Button>
      </div>
      {editor.stale && <div className="asb-banner asb-banner-warning" role="alert">
        <p>{t("modelRoutes.stale")}</p><Button variant="secondary" disabled={disabled} onClick={editor.reload}>{t("modelRoutes.reload")}</Button>
      </div>}
      <RouteEditor editor={editor} operation={operation} inventory={props.inventory} disabled={disabled} inspectorId={`${id}-inspector`} />
      {operation.error != null && <p className="asb-fail-text" role="alert">{errorText(operation.error, t)}</p>}
      {operation.releaseError != null && <p className="asb-field-help" role="status">{t("operations.save.retryRelease")}</p>}
    </div>
    {props.active && <RouteConfirmation operation={operation} onDocument={editor.setDocument} busy={props.busy} />}
    {props.active && editor.confirmation}
  </section>;
}
