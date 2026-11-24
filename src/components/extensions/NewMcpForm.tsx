import { Save } from "lucide-react";
import { useId, type RefObject } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { WorkspaceSubpage } from "../WorkspaceSubpage";
import { useUnsavedChanges } from "../useUnsavedChanges";
import { McpClientFields, McpIdentityFields, McpJsonField } from "./mcp-create/McpFormFields";
import { McpWizard } from "./mcp-create/McpWizard";
import { McpMetadataFields } from "./mcp-create/McpMetadataFields";
import { useMcpForm, type NewMcpFormProps } from "./mcp-create/useMcpForm";

export function NewMcpForm(props: NewMcpFormProps & { marketReturnFocus?: RefObject<HTMLButtonElement | null> }) {
  const { t } = useI18n();
  const form = useMcpForm(props);
  const id = useId();
  const leave = useUnsavedChanges(form.dirty, form.busy, { guardPageRoot: true });
  const close = () => leave.request(props.onCancel);
  if (form.wizard) {
    return <McpWizard initial={form.wizard} busy={form.busy}
      onApply={form.applyWizard} onCancel={form.closeWizard} />;
  }
  return (
    <WorkspaceSubpage layout="fixed-actions" title={t("extensions.toolbar.addMcp")}
      backLabel={t(props.marketReturnFocus ? "extensions.registry.backToMarket" : "extensions.backToLibrary")}
      returnFocusRef={props.marketReturnFocus} busy={form.busy} onBack={close}
      footer={<><Button variant="secondary" disabled={form.busy} onClick={close}>{t("confirm.cancel")}</Button>
        <Button type="submit" form={id} variant="primary" disabled={form.busy}><Save size={16} aria-hidden="true" />
          {form.saving ? t("mcp.action.saving") : form.clients.length ? t("mcp.action.saveAndEnable") : t("mcp.action.saveOnly")}</Button></>}>
    <form id={id} className="asb-form asb-mcp-create" aria-label={t("mcp.form.newAria")} aria-busy={form.busy} noValidate
      aria-describedby={form.error ? `${id}-error` : undefined}
      onSubmit={(event) => { event.preventDefault(); void form.submit(); }}>
      {props.registryEntry && <p>{t("extensions.registry.sourceVersion", { name: props.registryEntry.name, version: props.registryEntry.version })}</p>}
      <McpIdentityFields name={form.name} preset={form.preset} busy={form.busy}
        onNameChange={form.changeName} onPresetChange={form.selectPreset} />
      <McpMetadataFields value={form.metadata} busy={form.busy} onChange={form.changeMetadata} />
      <McpClientFields clients={form.clients} busy={form.busy} onToggle={form.toggleClient} />
      <McpJsonField value={form.json} busy={form.busy} focus={form.focusJson} invalid={Boolean(form.error)} errorId={`${id}-error`}
        onChange={form.changeJson} onWizard={form.openWizard} />
      {form.error && <p id={`${id}-error`} className="asb-warn-text asb-mcp-error" role="alert">{form.error}</p>}
    </form>{leave.confirmation}</WorkspaceSubpage>
  );
}
