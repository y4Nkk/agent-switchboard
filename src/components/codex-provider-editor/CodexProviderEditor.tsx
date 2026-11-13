import { useId, useState } from "react";
import type { AppKind, CodexProviderDraft, ProviderDraft, ProviderRecord, LocalizedMessage } from "../../api/client";
import type { CodexEditorSource } from "../../app/useProviders";
import { useI18n } from "../../i18n";
import { ProviderConversionSummary } from "../provider-editor/ProviderConversionSummary";
import { Checkbox } from "../Checkbox";
import { ProviderConnectionTest } from "../provider-editor/ProviderConnectionTest";
import { ProviderDiagnosticsEntry } from "../provider-diagnostics/ProviderDiagnosticsEntry";
import { ProviderEditorFrame } from "../provider-editor/ProviderEditorFrame";
import { ProviderAdvancedSettings } from "../provider-editor/ProviderAdvancedSettings";
import { ProviderNotesField } from "../provider-editor/ProviderIdentityFields";
import { ParametersLoadStatus } from "../provider-editor/ProviderParametersPage";
import { ResponsesOptionsFields } from "../provider-editor/ResponsesOptionsFields";
import { ProviderEditorStale } from "../provider-editor/ProviderEditorStale";
import { ProviderEditorTabs } from "../provider-editor/ProviderEditorTabs";
import { ProviderRoutingPanel } from "../provider-editor/ProviderRoutingPanel";
import { useProviderEditorSections, validateEditorForm, type ProviderEditorNavigation } from "../provider-editor/useProviderEditorSections";
import { routeIssueRole, type RouteDocument } from "../model-routing/model";
import { CodexCapabilitiesSection } from "./CodexCapabilitiesSection";
import { CodexConnectionFields } from "./CodexConnectionFields";
import { CodexAccessMode, CodexIdentityFields } from "./CodexIdentityFields";
import { CodexModelSection } from "./CodexModelSection";
import { CodexOfficialProviderForm } from "./CodexOfficialProviderForm";
import { CodexParametersPage } from "./CodexParametersPage";
import { useCodexProviderEditor, type CodexEditorState } from "./useCodexProviderEditor";
import { useCodexOfficialEditor } from "./useCodexOfficialEditor";
import "../../styles/base/provider-editor.css";

interface Props extends ProviderEditorNavigation {
  active: boolean;
  source: CodexEditorSource | null;
  busy: boolean;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
  onSave: (draft: CodexProviderDraft) => void;
  onSaveOfficial: (draft: ProviderDraft) => void;
  onSwitchAccessMode: (official: boolean, existing: ProviderRecord | null) => void;
  onCancel: () => void;
  onSwitchClient: (app: AppKind) => void;
}

type Navigation = ReturnType<typeof useProviderEditorSections>;
type OfficialProps = Props & { source: Extract<CodexEditorSource, { kind: "official" }> };

function ConnectionSection({ editor, props }: { editor: CodexEditorState; props: Props }) {
  const { draft, setDraft } = editor;
  const editing = props.source?.kind === "record";
  return <>
    {!editing && !editor.conversion && <CodexAccessMode busy={props.busy} onSwitchAccessMode={(official) => props.onSwitchAccessMode(official, null)} />}
    {editor.conversion && <ProviderConversionSummary conversion={editor.conversion} busy={props.busy}
      usageQuery={draft.usageQuery} onUsageChange={(usageQuery) => setDraft((current) => ({ ...current, usageQuery }))} />}
    <CodexIdentityFields draft={draft} busy={props.busy} editing={editing} setDraft={setDraft} onSwitchClient={props.onSwitchClient} />
    <CodexConnectionFields editor={editor} busy={props.busy} />
    <ProviderConnectionTest app="codex" busy={props.busy} active={props.active}
      baseUrl={draft.endpoint} apiKey={draft.apiKey} upstreamProtocol={draft.upstream}
      connection={draft.connection} authentication={draft.authentication}
      responsesOptions={draft.upstream === "responses" ? { requestMode: draft.requestMode } : null} defaultModel={draft.defaultModel} />
    <ProviderAdvancedSettings>
      {draft.upstream === "responses" && <ResponsesOptionsFields busy={props.busy} options={{ requestMode: draft.requestMode }}
        onChange={(next) => setDraft((current) => ({ ...current, requestMode: next.requestMode }))} />}
      <ProviderNotesField busy={props.busy} value={draft.notes} onChange={(notes) => setDraft((current) => ({ ...current, notes }))} />
    </ProviderAdvancedSettings>
  </>;
}

function ConversionReview({ editor, busy }: { editor: CodexEditorState; busy: boolean }) {
  const { t } = useI18n();
  if (!editor.conversion) return null;
  return <section className="asb-editor-section" aria-label={t("providers.conversion.summary")}>
    <div className="asb-editor-section-fields">
      {!editor.limitsComplete && <p className="asb-field-error" role="status">{t("providers.conversion.limitsRequired")}</p>}
      <p className="asb-field-help">{t("providers.conversion.capabilitiesRequired")}</p>
      <Checkbox label={t("providers.conversion.capabilitiesConfirm")} checked={editor.capabilitiesReviewed}
        disabled={busy} onChange={editor.setCapabilitiesReviewed} />
    </div>
  </section>;
}

function CodexForm({ editor, props, formId, navigation, route, onChange, catalogOpen, onCatalogOpen, revealRevision, onSubmit }: {
  editor: CodexEditorState; props: Props; formId: string; navigation: Navigation; route: RouteDocument;
  onChange: (document: RouteDocument) => void; catalogOpen: boolean; onCatalogOpen: (open: boolean) => void; revealRevision: number;
  onSubmit: (form: HTMLFormElement) => void;
}) {
  const { t } = useI18n();
  const { section, leave } = navigation;
  return <form id={formId} noValidate className="asb-provider-form" aria-label={t("codex.editor.editTitle")}
    onSubmit={(event) => { event.preventDefault(); onSubmit(event.currentTarget); }}>
    <div className="asb-provider-form asb-provider-editor-section" role="tabpanel" id={`${formId}-connection-panel`}
      aria-labelledby={`${formId}-connection-tab`} data-editor-section="connection" hidden={section !== "connection"}>
      <ConnectionSection editor={editor} props={{ ...props, active: props.active && section === "connection",
        onSwitchClient: (app) => leave.request(() => props.onSwitchClient(app)),
        onSwitchAccessMode: (official, existing) => leave.request(() => props.onSwitchAccessMode(official, existing)) }} />
    </div>
    <div className="asb-provider-form asb-provider-editor-section" role="tabpanel" id={`${formId}-models-panel`}
      aria-labelledby={`${formId}-models-tab`} data-editor-section="models" hidden={section !== "models"}>
      <ProviderRoutingPanel document={route} inventory={props.inventory} role={navigation.role} onRole={navigation.setRole}
        onChange={onChange} busy={props.busy} />
      <CodexModelSection editor={editor} busy={props.busy} userConfigModel={props.userConfigModel}
        userConfigWarnings={props.userConfigWarnings} open={catalogOpen} onOpenChange={onCatalogOpen} revealRevision={revealRevision} />
      <CodexCapabilitiesSection editor={editor} busy={props.busy} />
      <ConversionReview editor={editor} busy={props.busy} />
      {editor.problems.length > 0 && <div className="asb-field-error" role="status">
        {editor.problems.map((problem) => <p key={problem}>{problem}</p>)}
      </div>}
    </div>
    <div className="asb-provider-editor-section" role="tabpanel" id={`${formId}-parameters-panel`}
      aria-labelledby={`${formId}-parameters-tab`} data-editor-section="parameters" hidden={section !== "parameters"}>
      <CodexParametersPage editor={editor} busy={props.busy}
        baselineValues={props.source?.kind === "record" ? props.source.record.parameters.settings : undefined} />
    </div>
    <ParametersLoadStatus busy={props.busy} ready={editor.parameters.ready} error={editor.parameters.error} retry={editor.parameters.retry} />
  </form>;
}

function CodexProviderEditorSession(props: Props) {
  const { t } = useI18n();
  const editor = useCodexProviderEditor(props.source, props.busy);
  const navigation = useProviderEditorSections({ ...props, app: "codex", official: false, dirty: editor.dirty, viewportRef: editor.viewportRef });
  const [catalogOpen, setCatalogOpen] = useState(!editor.draft.catalog.length || Boolean(editor.conversion));
  const [invalid, setInvalid] = useState(0);
  const formId = useId();
  const route: RouteDocument = { kind: "codex", profileId: props.profileId, draft: editor.draft };
  const change = (next: RouteDocument) => {
    if (next.kind === "codex") editor.setDraft((current) => ({ ...current, defaultModel: next.draft.defaultModel,
      subagentRoute: next.draft.subagentRoute ?? null }));
  };
  const submit = (form: HTMLFormElement) => {
    if (navigation.stale) return;
    if (!validateEditorForm(form, (section) => { navigation.selectSection(section); if (section === "models") { setCatalogOpen(true); setInvalid((revision) => revision + 1); } })) return;
    const issueRole = routeIssueRole(route, props.inventory);
    if (issueRole || !editor.canSave) {
      navigation.selectSection("models"); navigation.setRole(issueRole ?? "main"); setCatalogOpen(true); setInvalid((revision) => revision + 1); return;
    }
    setInvalid(0); editor.save(props.onSave);
  };
  const close = () => navigation.leave.request(props.onCancel);
  return <ProviderEditorFrame title={t(props.source?.kind === "record" ? "codex.editor.editTitle" : "codex.editor.newTitle")}
    titleRef={editor.headingRef} viewportRef={editor.viewportRef} backLabel={t(props.returnTo === "gateway" ? "providerEditor.backToGateway" : "codex.editor.backToProviders")}
    busy={props.busy} onBack={close} onCancel={close} formId={formId}
    canSave={editor.parameters.ready && !editor.connection.modelsBusy && !navigation.stale}
    primary={<ProviderEditorTabs scope={formId} section={navigation.section} onChange={navigation.selectSection} busy={props.busy} official={false} />}
    saveLabel={editor.conversion ? t("providers.conversion.save", { target: "Codex" }) : undefined}>
    <ProviderEditorStale stale={navigation.stale} busy={props.busy} onReload={() => props.onReload(navigation.section, navigation.role)} />
    <CodexForm editor={editor} props={props} formId={formId} navigation={navigation} route={route} onChange={change}
      catalogOpen={catalogOpen} onCatalogOpen={setCatalogOpen} revealRevision={invalid} onSubmit={submit} />
    {invalid > 0 && <p className="asb-field-error" role="alert">{t("providerEditor.invalidDraft")}</p>}
    {props.source?.kind === "record" && navigation.section === "connection" && <ProviderDiagnosticsEntry profileId={props.profileId}
      name={props.source.record.profile.name} active={props.active} disabled={props.busy} />}
    {props.active && navigation.leave.confirmation}
  </ProviderEditorFrame>;
}

function CodexOfficialSession(props: OfficialProps) {
  const { t } = useI18n();
  const record = props.source.record;
  const editor = useCodexOfficialEditor(record?.profile ?? null, props.busy);
  const navigation = useProviderEditorSections({ ...props, app: "codex", official: true, dirty: editor.dirty, viewportRef: editor.viewportRef });
  const formId = useId();
  const close = () => navigation.leave.request(props.onCancel);
  return <ProviderEditorFrame title={t(record ? "codex.official.editTitle" : "codex.official.newTitle")}
    titleRef={editor.headingRef} viewportRef={editor.viewportRef} backLabel={t(props.returnTo === "gateway" ? "providerEditor.backToGateway" : "codex.editor.backToProviders")}
    busy={props.busy} onBack={close} onCancel={close} formId={formId} canSave={editor.canSave && !navigation.stale}
    primary={<ProviderEditorTabs scope={formId} section={navigation.section} onChange={navigation.selectSection} busy={props.busy} official />}>
    <ProviderEditorStale stale={navigation.stale} busy={props.busy} onReload={() => props.onReload(navigation.section, navigation.role)} />
    <CodexOfficialProviderForm formId={formId} busy={props.busy} editing={Boolean(record)} editor={editor} section={navigation.section}
      onSubmit={(form) => { if (!navigation.stale && validateEditorForm(form, navigation.selectSection)) editor.save(props.onSaveOfficial); }}
      onSwitchAccessMode={() => navigation.leave.request(() => props.onSwitchAccessMode(false, record))} />
    {record && navigation.section === "connection" && <ProviderDiagnosticsEntry profileId={props.profileId} name={record.profile.name}
      active={props.active} disabled={props.busy} />}
    {props.active && navigation.leave.confirmation}
  </ProviderEditorFrame>;
}

export function CodexProviderEditor(props: Props) {
  if (props.source?.kind === "official") return <CodexOfficialSession key={props.profileId} {...props} source={props.source} />;
  return <CodexProviderEditorSession key={props.profileId} {...props} />;
}
