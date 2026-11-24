import { uiMessage } from "../i18n/errors";
import { useMessageState } from "../i18n/use-message-state";
import { useEffect, useRef, useState } from "react";

import { parseClaudeExtraConfiguration } from "../api/client";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { EditableCodePreview } from "./EditableCodePreview";
import type { ConfigurationEditState } from "./CurrentConfigurationEditor";

interface Props {
  extra: Record<string, unknown> | undefined;
  busy: boolean;
  onChange: (extra: Record<string, unknown>) => void;
  onEditStateChange: (state: ConfigurationEditState) => void;
}

function serialized(extra: Record<string, unknown> | undefined): string {
  return JSON.stringify(extra ?? {}, null, 2);
}

function leafCount(value: unknown): number {
  if (!value || typeof value !== "object" || Array.isArray(value)) return 1;
  const entries = Object.values(value);
  return entries.length === 0 ? 0 : entries.reduce<number>((total, child) => total + leafCount(child), 0);
}

export function ClaudeExtraConfigurationEditor({ extra, busy, onChange, onEditStateChange }: Props) {
  const { t } = useI18n();
  const source = serialized(extra);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(source);
  const [parsing, setParsing] = useState(false);
  const [error, setError] = useMessageState();
  const [notice, setNotice] = useMessageState();
  const revision = useRef(0);
  const count = Object.values(extra ?? {}).reduce<number>((total, value) => total + leafCount(value), 0);
  const dirty = editing && draft !== source;

  useEffect(() => {
    if (!editing) setDraft(source);
  }, [editing, source]);
  useEffect(() => { onEditStateChange({ dirty, working: parsing }); }, [dirty, onEditStateChange, parsing]);

  const begin = () => {
    setDraft(source); setError(null); setNotice(null); setEditing(true);
  };
  const discard = () => {
    revision.current += 1;
    setDraft(source); setError(null); setEditing(false);
  };
  const save = () => {
    if (busy || parsing || draft === source) return;
    const currentRevision = revision.current + 1;
    revision.current = currentRevision;
    setParsing(true); setError(null);
    void parseClaudeExtraConfiguration(draft).then((next) => {
      if (revision.current !== currentRevision) return;
      onChange(next);
      setEditing(false);
      setNotice(uiMessage("clientConfig.extra.savedNotice"));
    }).catch((caught: { message?: string }) => {
      if (revision.current === currentRevision) {
        setError(caught);
      }
    }).finally(() => {
      if (revision.current === currentRevision) setParsing(false);
    });
  };

  return (
    <section className="asb-client-claude-extra-editor" aria-label={t("clientConfig.extra.editorTitle")}>
      <div className="asb-client-claude-extra-editor-heading">
        <div>
          <h3 className="asb-section-title">{t("clientConfig.extra.editorTitle")}</h3>
          <p className="asb-field-help">
            {t("clientConfig.extra.editorHelp")}
          </p>
        </div>
        {!editing && <Button variant="secondary" disabled={busy} onClick={begin}>{t("clientConfig.extra.editButton")}</Button>}
      </div>
      {!editing && <p className="asb-field-help">{t("clientConfig.extra.countLine", { count })}</p>}
      {editing && <>
        <EditableCodePreview
          target={t("clientConfig.extra.draftTarget")}
          content={draft}
          disabled={busy || parsing}
          onChange={(content) => { setDraft(content); setError(null); setNotice(null); }}
        />
        <div className="asb-client-claude-extra-editor-actions">
          <Button variant="secondary" disabled={busy || parsing} onClick={discard}>{t("clientConfig.editor.discard")}</Button>
          <Button variant="primary" disabled={busy || parsing || draft === source} onClick={save}>
            {parsing ? t("clientConfig.extra.validating") : t("clientConfig.extra.saveDraft")}
          </Button>
        </div>
      </>}
      {notice && <p className="asb-field-help" role="status">{notice}</p>}
      {error && <p className="asb-field-error" role="alert">{error}</p>}
    </section>
  );
}
