import { useId } from "react";
import type { AppKind, CodexProviderDraft, ProviderDraft, ProviderRecord, LocalizedMessage } from "../../api/client";
import type { CodexEditorSource } from "../../app/useProviders";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { useUnsavedChanges } from "../useUnsavedChanges";
import { ProviderConnectionTest } from "../provider-editor/ProviderConnectionTest";
import { ProviderDiagnosticsEntry } from "../provider-diagnostics/ProviderDiagnosticsEntry";
import { ProviderEditorFrame } from "../provider-editor/ProviderEditorFrame";
import { ProviderAdvancedSettings } from "../provider-editor/ProviderAdvancedSettings";
import { ProviderNotesField } from "../provider-editor/ProviderIdentityFields";
import { ParametersLoadStatus, ProviderParametersPage } from "../provider-editor/ProviderParametersPage";
import { ResponsesOptionsFields } from "../provider-editor/ResponsesOptionsFields";
import { CodexCapabilitiesSection } from "./CodexCapabilitiesSection";
import { CodexConnectionFields } from "./CodexConnectionFields";
import { CodexAccessMode, CodexIdentityFields } from "./CodexIdentityFields";
import { CodexModelSection } from "./CodexModelSection";
import { CodexOfficialProviderForm } from "./CodexOfficialProviderForm";
import { CodexParametersPage } from "./CodexParametersPage";
import { useCodexProviderEditor, type CodexEditorState } from "./useCodexProviderEditor";
import { useCodexOfficialEditor } from "./useCodexOfficialEditor";
import "../../styles/base/provider-editor.css";

interface Props {
  active: boolean;
  source: CodexEditorSource | null;
  busy: boolean;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
  onSave: (draft: CodexProviderDraft) => void;
  /** The official-login arm saves a generic draft: an official record is not
   * a third-party profile and never migrates into one. */
  onSaveOfficial: (draft: ProviderDraft) => void;
  /** Replaces the session when the user picks another access mode. */
  onSwitchAccessMode: (official: boolean, existing: ProviderRecord | null) => void;
  onCancel: () => void;
  /** Switching clients replaces the editor session; a Codex draft never
   * mutates into a Claude draft. */
  onSwitchClient: (app: AppKind) => void;
}

type OfficialProps = Omit<Props, "source"> & {
  source: Extract<CodexEditorSource, { kind: "official" }>;
};

function CodexProviderForm({ editor, formId, ...props }: Props & { editor: CodexEditorState; formId: string }) {
  const { t } = useI18n();
  const { draft, setDraft, parameters, setParametersOpen, triggerRef } = editor;
  const { busy, onSave } = props;
  const editing = props.source?.kind === "record";
  return (
    <form id={formId} className="asb-provider-form" aria-label={editing ? t("codex.editor.editTitle") : t("codex.editor.newTitle")}
      onSubmit={(event) => { event.preventDefault(); editor.save(onSave); }}>
      {!editing && <CodexAccessMode busy={busy}
        onSwitchAccessMode={(official) => props.onSwitchAccessMode(official, null)} />}
      <CodexIdentityFields draft={draft} busy={busy} editing={editing}
        setDraft={setDraft} onSwitchClient={props.onSwitchClient} />
      <CodexConnectionFields key={`connection-${draft.upstream}`} editor={editor} busy={busy} />
      <ProviderConnectionTest app="codex" busy={busy} active={props.active && !editor.parametersOpen}
        baseUrl={draft.endpoint} apiKey={draft.apiKey} upstreamProtocol={draft.upstream}
        connection={draft.connection} authentication={draft.authentication}
        responsesOptions={draft.upstream === "responses" ? { requestMode: draft.requestMode } : null}
        defaultModel={draft.defaultModel} />
      <CodexModelSection editor={editor} busy={busy}
        userConfigModel={props.userConfigModel} userConfigWarnings={props.userConfigWarnings} />
      <ProviderAdvancedSettings>
        {draft.upstream === "responses" && <ResponsesOptionsFields busy={busy}
          options={{ requestMode: draft.requestMode }}
          onChange={(next) => setDraft((current) => ({ ...current, requestMode: next.requestMode }))} />}
        <CodexCapabilitiesSection editor={editor} busy={busy} />
        <div className="asb-provider-advanced-action">
          <div><strong>{t("codex.editor.runtimeParams")}</strong><span>{t("providers.editor.parametersNote")}</span></div>
          <Button ref={triggerRef} variant="secondary" disabled={busy || !parameters.ready}
            onClick={() => setParametersOpen(true)}>{t("codex.editor.configureParams")} <span aria-hidden="true">→</span></Button>
        </div>
        <ParametersLoadStatus busy={busy} ready={parameters.ready}
          error={parameters.error} retry={parameters.retry} />
        <ProviderNotesField busy={busy} value={draft.notes}
          onChange={(value) => setDraft((current) => ({ ...current, notes: value }))} />
      </ProviderAdvancedSettings>
      {editor.problems.length > 0 && <div className="asb-field-error" role="alert">
        {editor.problems.map((problem) => <p key={problem}>{problem}</p>)}
      </div>}
    </form>
  );
}

function CodexProviderEditorSession(props: Props) {
  const { t } = useI18n();
  const editor = useCodexProviderEditor(props.source, props.busy);
  const leave = useUnsavedChanges(editor.dirty, props.busy);
  const close = () => leave.request(props.onCancel);
  const safeProps = { ...props,
    onSwitchClient: (app: AppKind) => leave.request(() => props.onSwitchClient(app)),
    onSwitchAccessMode: (official: boolean, existing: ProviderRecord | null) =>
      leave.request(() => props.onSwitchAccessMode(official, existing)),
  };
  const formId = useId();
  const parametersOpen = editor.parametersOpen;
  const editing = props.source?.kind === "record";
  const title = parametersOpen ? t("codex.editor.runtimeParams")
    : editing ? t("codex.editor.editTitle") : t("codex.editor.newTitle");
  const backLabel = parametersOpen ? t("codex.editor.backToEdit") : t("codex.editor.backToProviders");
  const goBack = () => parametersOpen ? editor.setParametersOpen(false) : close();
  return <ProviderEditorFrame title={title} titleRef={editor.headingRef} viewportRef={editor.viewportRef} backLabel={backLabel}
    busy={props.busy} onBack={goBack} onCancel={close} formId={formId} canSave={editor.canSave}>
    <div hidden={parametersOpen}>
      <CodexProviderForm {...safeProps} editor={editor} formId={formId} />
      {props.source?.kind === "record" && <ProviderDiagnosticsEntry profileId={props.source.record.profile.id}
        name={props.source.record.profile.name} active={props.active && !parametersOpen} disabled={props.busy} />}
    </div>
    {parametersOpen && <CodexParametersPage editor={editor} busy={props.busy}
      baselineValues={props.source?.kind === "record" ? props.source.record.parameters.settings : undefined} />}
    {leave.confirmation}
  </ProviderEditorFrame>;
}

function CodexOfficialSession(props: OfficialProps) {
  const { t } = useI18n();
  const record = props.source.record;
  const editor = useCodexOfficialEditor(record?.profile ?? null, props.busy);
  const leave = useUnsavedChanges(editor.dirty, props.busy);
  const close = () => leave.request(props.onCancel);
  const formId = useId();
  const { parametersOpen } = editor;
  const title = parametersOpen ? t("codex.editor.runtimeParams")
    : record ? t("codex.official.editTitle") : t("codex.official.newTitle");
  const backLabel = parametersOpen ? t("codex.editor.backToEdit") : t("codex.editor.backToProviders");
  const goBack = () => parametersOpen ? editor.setParametersOpen(false) : close();
  return <ProviderEditorFrame title={title} titleRef={editor.headingRef} viewportRef={editor.viewportRef} backLabel={backLabel} busy={props.busy}
    onBack={goBack} onCancel={close} formId={formId} canSave={editor.canSave}>
    <div hidden={parametersOpen}>
      <CodexOfficialProviderForm formId={formId} busy={props.busy} editing={Boolean(record)} editor={editor}
        onSubmit={() => editor.save(props.onSaveOfficial)}
        onSwitchAccessMode={() => leave.request(() => props.onSwitchAccessMode(false, record))} />
      {record && <ProviderDiagnosticsEntry profileId={record.profile.id} name={record.profile.name}
        active={props.active && !parametersOpen} disabled={props.busy} />}
    </div>
    {parametersOpen && <ProviderParametersPage value={editor.draft.parameters} parameters={editor.parameters}
      onChange={(parameters) => editor.setDraft((current) => ({ ...current, parameters }))} busy={props.busy}
      baselineValues={record?.profile.parameters.settings} />}
    {leave.confirmation}
  </ProviderEditorFrame>;
}

/** One Codex editor owns one session per access mode: a third-party draft and
 * an official-login record never convert into each other. */
export function CodexProviderEditor(props: Props) {
  if (props.source?.kind === "official") {
    return <CodexOfficialSession key={props.source.record?.profile.id ?? "new-codex-official"}
      {...props} source={props.source} />;
  }
  const source = props.source;
  const key = source?.kind === "record" ? source.record.profile.id : "new-codex";
  return <CodexProviderEditorSession key={key} {...props} />;
}
