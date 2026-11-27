import { Input } from "../Input";
import { SettingsGroup } from "../SettingsLayout";
import { OfficialLoginPanel } from "../OfficialLoginPanel";
import { useI18n } from "../../i18n";
import { ProviderAdvancedSettings } from "../provider-editor/ProviderAdvancedSettings";
import { ProviderNotesField } from "../provider-editor/ProviderIdentityFields";
import { ParametersLoadStatus, ProviderParametersPage } from "../provider-editor/ProviderParametersPage";
import { Button } from "../Button";
import type { ProviderEditorSection } from "../provider-editor/useProviderEditorSections";
import type { useCodexOfficialEditor } from "./useCodexOfficialEditor";

interface Props {
  formId: string;
  busy: boolean;
  editing: boolean;
  section: ProviderEditorSection;
  editor: ReturnType<typeof useCodexOfficialEditor>;
  onSubmit: (form: HTMLFormElement) => void;
  onSwitchAccessMode: () => void;
}

function BasicDetails({ busy, editing, editor, onSwitchAccessMode }: Pick<Props, "busy" | "editing" | "editor" | "onSwitchAccessMode">) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  return <SettingsGroup title={t("codex.identity.title")}>
    <div className="asb-editor-section-fields">
      <div className="asb-provider-field-grid">
        <div className="asb-field"><span>{t("codex.identity.accessMode")}</span>
          <p className="asb-provider-identity-value">{t("codex.identity.official")}</p>
          {!editing && <Button variant="ghost" disabled={busy} onClick={onSwitchAccessMode}>{t("providerSetup.thirdParty")}</Button>}
        </div>
        <label className="asb-field"><span>{t("codex.identity.name")}</span>
          <Input value={draft.name} required disabled={busy} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} />
        </label>
      </div>
      <label className="asb-field"><span>{t("codex.identity.website")}</span>
        <Input type="url" value={draft.websiteUrl ?? ""} disabled={busy} placeholder={t("codex.optional")}
          onChange={(event) => setDraft((current) => ({ ...current, websiteUrl: event.target.value }))} />
      </label>
    </div>
  </SettingsGroup>;
}

function SubscriptionSettings({ busy, editor }: Pick<Props, "busy" | "editor">) {
  const { t } = useI18n();
  return <div className="asb-provider-advanced-group">
    <div className="asb-field"><span>{t("codex.official.quotaInterval")}</span>
      <Input type="number" min={0} step={1} disabled={busy} value={editor.draft.officialQuotaRefreshIntervalMinutes ?? 0}
        aria-label={t("codex.official.quotaInterval")}
        onChange={(event) => {
          const parsed = Number(event.target.value.trim());
          editor.setDraft((current) => ({ ...current, officialQuotaRefreshIntervalMinutes: Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null }));
        }} />
    </div>
    <p className="asb-scope-note">{t("codex.official.quotaIntervalNote")}</p>
  </div>;
}

export function CodexOfficialProviderForm(props: Props) {
  const { t } = useI18n();
  const { formId, busy, editor, section } = props;
  const { draft, setDraft, parameters } = editor;
  return <form id={formId} noValidate className="asb-provider-form"
    onSubmit={(event) => { event.preventDefault(); props.onSubmit(event.currentTarget); }}>
    <div className="asb-provider-form asb-provider-editor-section" role="tabpanel" id={`${formId}-connection-panel`}
      aria-labelledby={`${formId}-connection-tab`} data-editor-section="connection" hidden={section !== "connection"}>
      <BasicDetails {...props} />
      <SettingsGroup title={t("codex.official.title")}>
        <OfficialLoginPanel app="codex" />
      </SettingsGroup>
      <ProviderAdvancedSettings>
        <SubscriptionSettings busy={busy} editor={editor} />
        <ProviderNotesField busy={busy} value={draft.notes ?? ""} onChange={(notes) => setDraft((current) => ({ ...current, notes }))} />
      </ProviderAdvancedSettings>
    </div>
    <div className="asb-provider-editor-section" role="tabpanel" id={`${formId}-parameters-panel`}
      aria-labelledby={`${formId}-parameters-tab`} data-editor-section="parameters" hidden={section !== "parameters"}>
      <ProviderParametersPage value={draft.parameters} parameters={parameters} busy={busy}
        baselineValues={props.editing ? editor.baselineValues : undefined}
        onChange={(value) => setDraft((current) => ({ ...current, parameters: value }))} />
    </div>
    <ParametersLoadStatus busy={busy} ready={parameters.ready} error={parameters.error} retry={parameters.retry} />
  </form>;
}
