import { useId } from "react";
import type { AppKind, ProviderDraft, ProviderProfile, LocalizedMessage } from "../api/client";
import { useI18n } from "../i18n";
import { useUnsavedChanges } from "./useUnsavedChanges";
import { Button } from "./Button";
import { OfficialLoginPanel } from "./OfficialLoginPanel";
import { ProviderDiagnosticsEntry } from "./provider-diagnostics/ProviderDiagnosticsEntry";
import { MainModelField } from "./provider-editor/MainModelField";
import { AvailableModelsField } from "./provider-editor/AvailableModelsField";
import { ProviderAdvancedSettings } from "./provider-editor/ProviderAdvancedSettings";
import { ProviderConnectionFields } from "./provider-editor/ProviderConnectionFields";
import { ProviderConnectionTest } from "./provider-editor/ProviderConnectionTest";
import { ProviderEditorFrame } from "./provider-editor/ProviderEditorFrame";
import {
  ProviderAccessMode,
  ProviderIdentityFields,
  ProviderNotesField,
} from "./provider-editor/ProviderIdentityFields";
import { ParametersLoadStatus, ProviderParametersPage } from "./provider-editor/ProviderParametersPage";
import { ResponsesOptionsFields } from "./provider-editor/ResponsesOptionsFields";
import { useProviderEditor, type ProviderEditorState } from "./provider-editor/useProviderEditor";
import "../styles/base/provider-editor.css";

interface Props {
  active: boolean;
  profile: ProviderProfile | null;
  initialApp: AppKind;
  busy: boolean;
  officialTakenApps: AppKind[];
  onOpenOfficial: (app: AppKind) => void;
  /** Replaces the editor session when the user picks the other client. */
  onSwitchClient: (app: AppKind) => void;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
  onSave: (draft: ProviderDraft) => void;
  onCancel: () => void;
}

function ModelSection({ editor, busy, userConfigModel, userConfigWarnings }:
  Pick<Props, "busy" | "userConfigModel" | "userConfigWarnings"> & { editor: ProviderEditorState }) {
  const { t } = useI18n();
  const { draft, setDraft, connection } = editor;
  const claudeSettings = draft.modelOptions?.kind === "claude" ? draft.modelOptions : null;
  return (
    <section className="asb-editor-section" aria-label={t("providers.editor.section.model")}>
      <h3 className="asb-section-title">{t("providers.editor.section.model")}</h3>
      <div className="asb-editor-section-fields">
        <MainModelField draft={draft} busy={busy} baseUrl={connection.baseUrl}
          claudeSettings={claudeSettings}
          models={connection.models} modelsBusy={connection.modelsBusy} modelsError={connection.modelsError}
          modelsEndpointError={connection.modelsEndpointError}
          userConfigModel={userConfigModel} userConfigWarnings={userConfigWarnings}
          fetchModels={connection.fetchModels} setDraft={setDraft} />
        <AvailableModelsField draft={draft} busy={busy} setDraft={setDraft} />
      </div>
    </section>
  );
}

function AdvancedSettings({ editor, ...props }: Props & { editor: ProviderEditorState }) {
  const { t } = useI18n();
  const { draft, setDraft, parameters, setParametersOpen, triggerRef } = editor;
  return <ProviderAdvancedSettings>
    {draft.upstreamProtocol === "responses" && <ResponsesOptionsFields busy={props.busy}
      options={draft.responsesOptions}
      onChange={(next) => setDraft((current) => ({ ...current, responsesOptions: next }))} />}
    <div className="asb-provider-advanced-action">
      <div><strong>{t("providers.editor.parameters")}</strong><span>{t("providers.editor.parametersNote")}</span></div>
      <Button ref={triggerRef} variant="secondary" disabled={props.busy || !parameters.ready}
        onClick={() => setParametersOpen(true)}>{t("providers.editor.configureParameters")} <span aria-hidden="true">→</span></Button>
    </div>
    <ParametersLoadStatus busy={props.busy} ready={parameters.ready}
      error={parameters.error} retry={parameters.retry} />
    <ProviderNotesField key={`notes-${draft.app}`} busy={props.busy} value={draft.notes ?? null}
      onChange={(value) => setDraft((current) => ({ ...current, notes: value }))} />
  </ProviderAdvancedSettings>;
}

function ProviderForm({ editor, formId, ...props }: Props & { editor: ProviderEditorState; formId: string }) {
  const { t } = useI18n();
  const { draft } = editor;
  const { busy, profile, onSave } = props;
  const official = draft.routeMode === "official";
  return (
    <form id={formId} className="asb-provider-form" aria-label={profile ? t("providers.editor.title.edit") : t("providers.workspace.newProvider")}
      onSubmit={(event) => { event.preventDefault(); editor.save(onSave); }}>
      {!profile && <ProviderAccessMode editor={editor} busy={busy} officialTakenApps={props.officialTakenApps}
        onOpenOfficial={props.onOpenOfficial} />}
      <ProviderIdentityFields editor={editor} busy={busy} editing={Boolean(profile)}
        onSwitchClient={props.onSwitchClient} />
      {!official && <>
        <ProviderConnectionFields key={`connection-${draft.app}`} editor={editor} busy={busy} />
        <ProviderConnectionTest app={draft.app} busy={busy} active={props.active && !editor.parametersOpen}
          baseUrl={draft.baseUrl} apiKey={draft.apiKey} upstreamProtocol={draft.upstreamProtocol}
          connection={draft.connection} authentication={draft.authentication}
          responsesOptions={draft.responsesOptions} defaultModel={draft.model} />
        <ModelSection editor={editor} busy={busy}
          userConfigModel={props.userConfigModel} userConfigWarnings={props.userConfigWarnings} />
      </>}
      {official && <section className="asb-editor-section" aria-label={t("providers.label.officialLogin")}>
        <h3 className="asb-section-title">{t("providers.label.officialLogin")}</h3>
        <div className="asb-editor-section-fields"><OfficialLoginPanel app={draft.app} onFinished={editor.setLoginDone} /></div>
      </section>}
      <AdvancedSettings {...props} editor={editor} />
    </form>
  );
}

function ProviderEditorSession(props: Props) {
  const { t } = useI18n();
  const editor = useProviderEditor(props.profile, props.initialApp, props.busy);
  const leave = useUnsavedChanges(editor.dirty, props.busy);
  const close = () => leave.request(props.onCancel);
  const safeProps = { ...props,
    onSwitchClient: (app: AppKind) => leave.request(() => props.onSwitchClient(app)),
    onOpenOfficial: (app: AppKind) => leave.request(() => props.onOpenOfficial(app)),
  };
  const formId = useId();
  const parametersOpen = editor.parametersOpen;
  const title = parametersOpen ? t("providers.editor.parameters")
    : props.profile ? t("providers.editor.title.edit") : t("providers.workspace.newProvider");
  const backLabel = parametersOpen ? t("providers.editor.backToEditor") : t("providers.editor.backToProviders");
  const goBack = () => parametersOpen ? editor.setParametersOpen(false) : close();
  return <ProviderEditorFrame title={title} titleRef={editor.headingRef} backLabel={backLabel}
    busy={props.busy} onBack={goBack} onCancel={close} formId={formId} canSave={editor.canSave}>
    <div hidden={parametersOpen}>
      <ProviderForm {...safeProps} editor={editor} formId={formId} />
      {props.profile && <ProviderDiagnosticsEntry profileId={props.profile.id} name={props.profile.name}
        active={props.active && !parametersOpen} disabled={props.busy} />}
    </div>
    {parametersOpen && <ProviderParametersPage value={editor.draft.parameters} parameters={editor.parameters}
      onChange={(parameters) => editor.setDraft((current) => ({ ...current, parameters }))} busy={props.busy}
      claudeFragment={editor.draft.claudeFragment}
      onFragmentChange={(claudeFragment) => editor.setDraft((current) => ({ ...current, claudeFragment }))}
      baselineValues={props.profile?.parameters.settings} />}
    {leave.confirmation}
  </ProviderEditorFrame>;
}

/** Each profile owns one draft across both editor levels and the save transaction. */
export function ProviderEditor(props: Props) {
  return <ProviderEditorSession key={props.profile?.id ?? props.initialApp} {...props} />;
}
