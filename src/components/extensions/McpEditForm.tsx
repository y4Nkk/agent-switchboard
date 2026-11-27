import { useId, useRef, useState } from "react";
import { Save, X } from "lucide-react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { WorkspaceSubpage } from "../WorkspaceSubpage";
import { SettingsGroup, SettingsLayout } from "../SettingsLayout";
import { useUnsavedChanges } from "../useUnsavedChanges";
import { McpClientFields, McpIdentityFields, McpJsonField } from "./mcp-create/McpFormFields";
import { McpMetadataFields } from "./mcp-create/McpMetadataFields";
import { McpWizard } from "./mcp-create/McpWizard";
import { useMcpEditForm, type McpEditFormProps } from "./mcp-edit/useMcpEditForm";

export type { McpEditFormProps } from "./mcp-edit/useMcpEditForm";

export function McpEditForm(props: McpEditFormProps) {
  const [loaded, setLoaded] = useState({ envelope: props.envelope, generation: 0 });
  const [loading, setLoading] = useState(false);
  const running = useRef(false);
  const reload = async () => {
    if (running.current || props.busy) return;
    running.current = true; setLoading(true);
    try {
      const envelope = await props.onReload();
      if (envelope) setLoaded((current) => ({ envelope, generation: current.generation + 1 }));
    } finally { running.current = false; setLoading(false); }
  };
  return <McpEditor key={`${props.envelope.id}:${loaded.generation}`} {...props}
    envelope={loaded.envelope} busy={props.busy || loading} reload={reload} />;
}

function McpEditor(props: McpEditFormProps & { reload: () => Promise<void> }) {
  const { t } = useI18n();
  const initial = useRef(props.envelope);
  const form = useMcpEditForm({ ...props, envelope: initial.current });
  const stale = props.currentRevision !== initial.current.revision;
  const id = useId();
  const leave = useUnsavedChanges(form.dirty, form.busy, { guardPageRoot: true });
  const close = () => leave.request(props.onCancel);
  if (form.wizard) return <McpWizard initial={form.wizard} original={form.original} busy={form.busy}
    onApply={form.applyWizard} onCancel={form.closeWizard} />;
  return <WorkspaceSubpage layout="fixed-actions" title={t("extensions.editor.editMcp", { name: props.envelope.name })}
    backLabel={t("extensions.backToLibrary")} onBack={close} busy={form.busy}
    footer={<><Button variant="secondary" disabled={form.busy} onClick={close}><X size={16} aria-hidden="true" />{t("confirm.cancel")}</Button>
      <Button type="submit" form={id} variant="primary" disabled={form.busy || stale}><Save size={16} aria-hidden="true" />
        {form.saving ? t("mcp.edit.saving") : t("mcp.edit.save")}</Button></>}>
    <SettingsLayout>
      <form id={id} className="asb-form asb-mcp-create" aria-label={t("mcp.edit.aria")} aria-busy={form.busy} noValidate
        aria-describedby={form.error ? `${id}-error` : undefined}
        onSubmit={(event) => { event.preventDefault(); if (!stale) void form.submit(); }}>
        {stale && <div className="asb-banner" role="alert">{t("extensions.workbench.sourceChanged")}
          <Button variant="secondary" disabled={form.busy}
            onClick={() => leave.request(() => void props.reload())}>{t("extensions.library.reload")}</Button></div>}
        <SettingsGroup title={t("mcp.group.identity")}>
          <McpIdentityFields name={form.name} busy={form.busy} registry={props.envelope.mcpMetadata?.registry} onNameChange={form.changeName} />
          <McpMetadataFields value={form.metadata} busy={form.busy} onChange={form.changeMetadata} />
        </SettingsGroup>
        <McpJsonField value={form.json} busy={form.busy} focus={form.focusJson} error={form.error} errorId={`${id}-error`}
          onChange={form.changeJson} onWizard={form.openWizard} />
        {props.initialClients && <McpClientFields clients={form.clients} busy={form.busy} onToggle={form.toggleClient} />}
      </form>
    </SettingsLayout>
    {leave.confirmation}
  </WorkspaceSubpage>;
}
