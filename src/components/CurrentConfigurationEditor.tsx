import { useMessageState } from "../i18n/use-message-state";
import { useEffect, useRef, useState, type ReactNode } from "react";

import {
  commitClientConfigurationRepair,
  commitManualClientConfiguration,
  previewClientConfigurationRepair,
  previewManualClientConfiguration,
  type AppKind,
  type ClientConfigurationApplyPreview,
  type ClientConfigurationRepairPreview,
  type CodexSubagentSettings,
  type CurrentClientConfiguration,
} from "../api/client";
import { clientSettingsPayload } from "../app/claude-common-settings";
import type { ClientSettingsEditorState } from "../app/useClientSettings";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { CodePreview } from "./CodePreview";
import { EditableCodePreview } from "./EditableCodePreview";
import { EditIcon } from "./icons";

export interface ConfigurationEditState { dirty: boolean; working: boolean }

interface Props {
  app: AppKind;
  busy: boolean;
  editorState: ClientSettingsEditorState;
  source: CurrentClientConfiguration;
  subagentDraft?: CodexSubagentSettings;
  onApplied: () => void;
  /** Reports whether an unapplied edit would be lost or a write is running,
   * so the owning dialog can guard its close and lock while writing. */
  onEditStateChange: (state: ConfigurationEditState) => void;
}

function currentSettings({ app, editorState }: Pick<Props, "app" | "editorState">) {
  if (!editorState.editor || !editorState.draft) return null;
  return clientSettingsPayload(app, editorState.draft, editorState.claudeExtra);
}

function CurrentConfigurationRepair({ app, busy, source, onApplied, onEditStateChange }: Pick<Props,
  "app" | "busy" | "source" | "onApplied" | "onEditStateChange">) {
  const { t } = useI18n();
  const [preview, setPreview] = useState<ClientConfigurationRepairPreview | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  useEffect(() => { setPreview(null); setError(null); }, [source.contentHash]);
  useEffect(() => { onEditStateChange({ dirty: false, working }); }, [working, onEditStateChange]);
  const prepare = () => {
    if (busy || working) return;
    setWorking(true); setError(null); setPreview(null);
    void previewClientConfigurationRepair(app, source.contentHash).then(setPreview)
      .catch((caught: { message?: string }) => setError(caught))
      .finally(() => setWorking(false));
  };
  const commit = () => {
    if (!preview || busy || working) return;
    setWorking(true); setError(null);
    void commitClientConfigurationRepair(app, source.contentHash, preview.file.renderedHash, preview.targetExisted)
      .then(() => { setPreview(null); onApplied(); })
      .catch((caught: { message?: string }) => setError(caught))
      .finally(() => setWorking(false));
  };
  return <>
    <p className="asb-field-error" role="alert">{t("clientConfig.repair.invalidSyntax", { reason: source.syntaxError ?? t("clientConfig.repair.unparseable") })}</p>
    {!preview ? <div className="asb-client-configuration-file-actions">
      <Button variant="secondary" disabled={busy || working} onClick={prepare}>
        {working ? t("clientConfig.repair.generating") : t("clientConfig.repair.button")}
      </Button>
    </div> : <>
      <CodePreview target={t("clientConfig.repair.candidateTarget", { target: preview.file.preview.target })} content={preview.file.content} />
      <div className="asb-client-configuration-file-actions">
        <Button variant="secondary" disabled={busy || working} onClick={() => setPreview(null)}>{t("clientConfig.common.back")}</Button>
        <Button variant="primary" disabled={busy || working} onClick={commit}>{working ? t("clientConfig.repair.fixing") : t("clientConfig.repair.confirm")}</Button>
      </div>
    </>}
    {error && <p className="asb-field-error" role="alert">{error}</p>}
  </>;
}

function useCurrentConfigurationEdit({ app, busy, editorState, source, subagentDraft, onApplied }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(source.content);
  const baseline = useRef(source);
  const [preview, setPreview] = useState<ClientConfigurationApplyPreview | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  const settings = currentSettings({ app, editorState });
  const stale = editing && baseline.current.contentHash !== source.contentHash;
  const canWrite = !stale && !!settings && (app !== "codex" || !!subagentDraft);
  useEffect(() => {
    setPreview(null);
    if (editing) return;
    baseline.current = source;
    setDraft(source.content); setError(null);
  }, [source.content, source.contentHash, editing]);
  const startEditing = () => { baseline.current = source; setEditing(true); };
  const changeDraft = (content: string) => { setDraft(content); setPreview(null); setError(null); };
  const prepare = () => {
    if (!settings || !canWrite || busy || working) return;
    setWorking(true); setError(null); setPreview(null);
    void previewManualClientConfiguration(app, source.contentHash, draft, settings, subagentDraft).then(setPreview)
      .catch((caught: { message?: string }) => setError(caught))
      .finally(() => setWorking(false));
  };
  const commit = () => {
    if (!settings || !canWrite || !preview || busy || working) return;
    setWorking(true); setError(null);
    void commitManualClientConfiguration(app, source.contentHash, preview.file.renderedHash, preview.settingsHash,
      preview.targetExisted, draft, settings, subagentDraft).then(() => { setEditing(false); setPreview(null); onApplied(); })
      .catch((caught: { message?: string }) => setError(caught))
      .finally(() => setWorking(false));
  };
  return { canWrite, changeDraft, commit, draft, editing, error, prepare, preview, startEditing, stale, setEditing, setPreview, setDraft, setError, working };
}

/** The common-configuration-file dialog's file module: one surface reviews
 * the redacted real client configuration and turns editable in place through
 * the edit icon in its heading, instead of a read-only preview above a
 * separate manual editor below. */
export function CurrentConfigurationEditor(props: Props) {
  const { app, busy, source, onEditStateChange } = props;
  const { t } = useI18n();
  const edit = useCurrentConfigurationEdit(props);
  const repairing = !source.syntaxOk && !edit.editing;
  const dirty = edit.editing && edit.draft !== source.content;
  useEffect(() => {
    if (!repairing) onEditStateChange({ dirty, working: edit.working });
  }, [dirty, edit.working, onEditStateChange, repairing]);
  const heading = (help: string, icon?: ReactNode) => (
    <div className="asb-client-configuration-file-heading">
      <div>
        <h3 className="asb-section-title">{t("clientConfig.editor.title")}</h3>
        <p className="asb-field-help">{help}</p>
      </div>
      {icon}
    </div>
  );
  if (repairing) {
    return (
      <section className="asb-client-configuration-file" aria-label={t("clientConfig.repair.sectionAria")}>
        {heading(t("clientConfig.repair.help"))}
        <CurrentConfigurationRepair app={app} busy={busy} source={source} onApplied={props.onApplied}
          onEditStateChange={onEditStateChange} />
      </section>
    );
  }
  return (
    <section className="asb-client-configuration-file" aria-label={t("clientConfig.editor.title")}>
      {heading(edit.editing || edit.preview ? t("clientConfig.editor.editHelp") : t("clientConfig.editor.reviewHelp"), !edit.editing && !edit.preview ? (
        <Button variant="icon" aria-label={t("clientConfig.editor.editAria")} disabled={busy} onClick={edit.startEditing}>
          <EditIcon />
        </Button>
      ) : undefined)}
      {edit.stale && <p className="asb-field-error" role="alert">{t("clientConfig.editor.sourceChanged")}</p>}
      {edit.preview ? <>
        <CodePreview target={t("clientConfig.editor.manualCandidateTarget", { target: edit.preview.file.preview.target })} content={edit.preview.file.content} />
        <div className="asb-client-configuration-file-actions">
          <Button variant="secondary" disabled={busy || edit.working} onClick={() => edit.setPreview(null)}>{t("clientConfig.editor.backToEdit")}</Button>
          <Button variant="primary" disabled={!edit.canWrite || busy || edit.working} onClick={edit.commit}>
            {edit.working ? t("clientConfig.editor.applying") : t("clientConfig.editor.confirmManual")}
          </Button>
        </div>
      </> : edit.editing ? <>
        <EditableCodePreview target={t("clientConfig.editor.targetLabel", { target: source.target })} content={edit.draft}
          disabled={busy || edit.working} onChange={edit.changeDraft} />
        <div className="asb-client-configuration-file-actions">
          <Button variant="secondary" disabled={busy || edit.working} onClick={() => {
            edit.setDraft(source.content); edit.setEditing(false); edit.setError(null);
          }}>{t("clientConfig.editor.discard")}</Button>
          <Button variant="primary" disabled={!edit.canWrite || edit.draft === source.content || busy || edit.working} onClick={edit.prepare}>
            {edit.working ? t("clientConfig.editor.generatingPreview") : t("clientConfig.editor.previewManual")}
          </Button>
        </div>
      </> : source.exists ? (
        <CodePreview target={t("clientConfig.editor.targetLabel", { target: source.target })} content={source.content} />
      ) : (
        <p className="asb-empty">{t("clientConfig.editor.missingFile", { target: source.target })}</p>
      )}
      {edit.error && <p className="asb-field-error" role="alert">{edit.error}</p>}
    </section>
  );
}
