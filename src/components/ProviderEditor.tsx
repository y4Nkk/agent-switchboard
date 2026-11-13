import { useId, useState } from "react";
import type { AppKind, ProviderDraft, ProviderProfile, ProviderConversion, LocalizedMessage } from "../api/client";
import { useI18n } from "../i18n";
import { localizedMessageText } from "../i18n/errors";
import { claudeOptions } from "../lib/provider-drafts";
import { OfficialLoginPanel } from "./OfficialLoginPanel";
import { ProviderDiagnosticsEntry } from "./provider-diagnostics/ProviderDiagnosticsEntry";
import { AvailableModelsField } from "./provider-editor/AvailableModelsField";
import { ProviderAdvancedSettings } from "./provider-editor/ProviderAdvancedSettings";
import { ProviderConnectionFields } from "./provider-editor/ProviderConnectionFields";
import { ProviderConnectionTest } from "./provider-editor/ProviderConnectionTest";
import { ProviderEditorFrame } from "./provider-editor/ProviderEditorFrame";
import { ProviderAccessMode, ProviderIdentityFields, ProviderNotesField } from "./provider-editor/ProviderIdentityFields";
import { ParametersLoadStatus, ProviderParametersPage } from "./provider-editor/ProviderParametersPage";
import { ResponsesOptionsFields } from "./provider-editor/ResponsesOptionsFields";
import { ProviderConversionSummary } from "./provider-editor/ProviderConversionSummary";
import { useProviderEditor, type ProviderEditorState } from "./provider-editor/useProviderEditor";
import { ProviderEditorStale } from "./provider-editor/ProviderEditorStale";
import { ProviderEditorTabs } from "./provider-editor/ProviderEditorTabs";
import { ProviderRoutingPanel } from "./provider-editor/ProviderRoutingPanel";
import { validateEditorForm, useProviderEditorSections, type ProviderEditorNavigation } from "./provider-editor/useProviderEditorSections";
import { routeIssueRole, type RouteDocument } from "./model-routing/model";
import { CollapsibleSection } from "./Collapsible";
import { Button } from "./Button";
import "../styles/base/provider-editor.css";

interface Props extends ProviderEditorNavigation {
  active: boolean;
  conversion?: ProviderConversion;
  sessionId?: string;
  profile: ProviderProfile | null;
  initialApp: AppKind;
  busy: boolean;
  officialTakenApps: AppKind[];
  onOpenOfficial: (app: AppKind) => void;
  onSwitchClient: (app: AppKind) => void;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
  onSave: (draft: ProviderDraft) => void;
  onCancel: () => void;
}

type Editor = ProviderEditorState;

function ConnectionSection({ editor, ...props }: Props & { editor: Editor }) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  const official = draft.routeMode === "official";
  return <>
    {!props.profile && !props.conversion && <ProviderAccessMode editor={editor} busy={props.busy}
      officialTakenApps={props.officialTakenApps} onOpenOfficial={props.onOpenOfficial} />}
    {props.conversion && <ProviderConversionSummary conversion={props.conversion} busy={props.busy}
      usageQuery={draft.usageQuery ?? null} onUsageChange={(usageQuery) => setDraft((current) => ({ ...current, usageQuery }))} />}
    <ProviderIdentityFields editor={editor} busy={props.busy} editing={Boolean(props.profile)} onSwitchClient={props.onSwitchClient} />
    {official ? <section className="asb-editor-section" aria-label={t("providers.label.officialLogin")}>
      <h3 className="asb-section-title">{t("providers.label.officialLogin")}</h3>
      <div className="asb-editor-section-fields"><OfficialLoginPanel app={draft.app} onFinished={editor.setLoginDone} /></div>
    </section> : <>
      <ProviderConnectionFields editor={editor} busy={props.busy} />
      <ProviderConnectionTest app={draft.app} busy={props.busy} active={props.active}
        baseUrl={draft.baseUrl} apiKey={draft.apiKey} upstreamProtocol={draft.upstreamProtocol}
        connection={draft.connection} authentication={draft.authentication}
        responsesOptions={draft.responsesOptions} defaultModel={draft.model} />
    </>}
    <ProviderAdvancedSettings>
      {draft.upstreamProtocol === "responses" && <ResponsesOptionsFields busy={props.busy} options={draft.responsesOptions}
        onChange={(responsesOptions) => setDraft((current) => ({ ...current, responsesOptions }))} />}
      <ProviderNotesField busy={props.busy} value={draft.notes ?? null}
        onChange={(notes) => setDraft((current) => ({ ...current, notes }))} />
    </ProviderAdvancedSettings>
  </>;
}

function ModelCatalog({ editor, busy }: { editor: Editor; busy: boolean }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const { setDraft, connection } = editor;
  const fetch = async () => {
    const models = await connection.fetchModels();
    if (models) setDraft((current) => ({ ...current, modelOptions: claudeOptions(current.modelOptions, {
      availableModels: [...new Set([...(current.modelOptions?.kind === "claude" ? current.modelOptions.availableModels ?? [] : []),
        ...models.map(({ id }) => id)])],
    }) }));
  };
  return <CollapsibleSection open={open} onOpenChange={setOpen} label={t("providers.editor.availableModels")}>
    <div className="asb-provider-form">
      <Button variant="secondary" disabled={busy || connection.modelsBusy || !connection.baseUrl || !!connection.modelsEndpointError}
        onClick={() => void fetch()}>{t("providerEditor.fetchModels")}</Button>
      {connection.modelsError && <p className="asb-warn-text" role="alert">{connection.modelsError}</p>}
      <AvailableModelsField busy={busy} setDraft={setDraft} text={editor.availableModelsText} onTextChange={editor.setAvailableModelsText} />
    </div>
  </CollapsibleSection>;
}

function ParametersSection({ editor, busy, profile }: { editor: Editor; busy: boolean; profile: ProviderProfile | null }) {
  return <ProviderParametersPage value={editor.draft.parameters} parameters={editor.parameters}
    onChange={(parameters) => editor.setDraft((current) => ({ ...current, parameters }))} busy={busy}
    claudeFragment={editor.draft.claudeFragment}
    onFragmentChange={(claudeFragment) => editor.setDraft((current) => ({ ...current, claudeFragment }))}
    baselineValues={profile?.parameters.settings} />;
}

function ProviderEditorSession(props: Props) {
  const { t } = useI18n();
  const editor = useProviderEditor(props.profile, props.initialApp, props.busy,
    props.conversion?.seed?.app === "claude" ? props.conversion.seed.draft : undefined);
  const official = editor.draft.routeMode === "official";
  const navigation = useProviderEditorSections({ ...props, app: editor.draft.app, official,
    dirty: editor.dirty, viewportRef: editor.viewportRef });
  const { section, selectSection, role, setRole, leave } = navigation;
  const [invalid, setInvalid] = useState(false);
  const formId = useId();
  const route: RouteDocument = { kind: "profile", profileId: props.profileId, draft: editor.draft };
  const changeRoute = (next: RouteDocument) => {
    if (next.kind === "profile") editor.setDraft((current) => ({ ...current, model: next.draft.model, modelOptions: next.draft.modelOptions }));
  };
  const submit = (form: HTMLFormElement) => {
    if (navigation.stale) return;
    if (!validateEditorForm(form, selectSection)) return;
    const issueRole = official ? null : routeIssueRole(route, props.inventory);
    if (issueRole) { selectSection("models"); setRole(issueRole); setInvalid(true); return; }
    setInvalid(false);
    editor.save(props.onSave);
  };
  const close = () => leave.request(props.onCancel);
  return <ProviderEditorFrame title={props.profile ? t("providers.editor.title.edit") : t("providers.workspace.newProvider")}
    titleRef={editor.headingRef} viewportRef={editor.viewportRef} backLabel={t(props.returnTo === "gateway" ? "providerEditor.backToGateway" : "providers.editor.backToProviders")}
    busy={props.busy} onBack={close} onCancel={close} formId={formId} canSave={editor.canSave && !navigation.stale}
    primary={<ProviderEditorTabs section={section} onChange={selectSection} scope={formId} busy={props.busy} official={official} />}
    saveLabel={props.conversion ? t("providers.conversion.save", { target: "Claude" }) : undefined}>
    <ProviderEditorStale stale={navigation.stale} busy={props.busy} onReload={() => props.onReload(section, role)} />
    <form id={formId} noValidate className="asb-provider-form" onSubmit={(event) => { event.preventDefault(); submit(event.currentTarget); }}>
      <div className="asb-provider-form asb-provider-editor-section" id={`${formId}-connection-panel`} role="tabpanel"
        aria-labelledby={`${formId}-connection-tab`} data-editor-section="connection" hidden={section !== "connection"}>
        <ConnectionSection {...props} editor={editor} active={props.active && section === "connection"}
          onSwitchClient={(app) => leave.request(() => props.onSwitchClient(app))}
          onOpenOfficial={props.onOpenOfficial} />
      </div>
      {!official && <div className="asb-provider-form asb-provider-editor-section" id={`${formId}-models-panel`} role="tabpanel"
        aria-labelledby={`${formId}-models-tab`} data-editor-section="models" hidden={section !== "models"}>
        <ProviderRoutingPanel document={route} inventory={props.inventory} role={role} onRole={setRole} onChange={changeRoute} busy={props.busy} />
        <ModelCatalog editor={editor} busy={props.busy} />
        {props.userConfigModel && <p className="asb-scope-note">{t("providers.editor.userConfigModel", { model: props.userConfigModel })}</p>}
        {props.userConfigWarnings.map((warning) => <p key={warning.key} className="asb-scope-note asb-warn-text">{localizedMessageText(warning, t)}</p>)}
      </div>}
      <div className="asb-provider-editor-section" id={`${formId}-parameters-panel`} role="tabpanel"
        aria-labelledby={`${formId}-parameters-tab`} data-editor-section="parameters" hidden={section !== "parameters"}>
        <ParametersSection editor={editor} busy={props.busy} profile={props.profile} />
      </div>
      <ParametersLoadStatus busy={props.busy} ready={editor.parameters.ready} error={editor.parameters.error} retry={editor.parameters.retry} />
      {invalid && <p className="asb-warn-text" role="alert">{t("providerEditor.invalidDraft")}</p>}
    </form>
    {props.profile && section === "connection" && <ProviderDiagnosticsEntry profileId={props.profileId} name={props.profile.name}
      active={props.active} disabled={props.busy} />}
    {props.active && leave.confirmation}
  </ProviderEditorFrame>;
}

export function ProviderEditor(props: Props) {
  return <ProviderEditorSession key={props.sessionId ?? props.profileId} {...props} />;
}
