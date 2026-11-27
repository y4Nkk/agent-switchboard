import type { AppKind } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { ClientLogo } from "../ClientLogo";
import { Input } from "../Input";
import { Button } from "../Button";
import { SettingsGroup } from "../SettingsLayout";
import { Textarea } from "../Textarea";
import { defaultConnection } from "./draft";
import type { ProviderEditorState } from "./useProviderEditor";

interface IdentityProps {
  editor: ProviderEditorState;
  busy: boolean;
}

interface AccessModeProps {
  editor: ProviderEditorState;
  busy: boolean;
  officialTakenApps: AppKind[];
  onOpenOfficial: (app: AppKind) => void;
}

export function ProviderAccessMode({ editor, busy, officialTakenApps, onOpenOfficial }: AccessModeProps) {
  const { t } = useI18n();
  const { draft, setDraft, setLoginDone } = editor;
  const official = draft.routeMode === "official";
  const change = () => {
    if (!official && officialTakenApps.includes(draft.app)) { onOpenOfficial(draft.app); return; }
    setDraft((current) => ({ ...current, routeMode: official ? "custom" : "official",
      ...defaultConnection(current.app), apiKey: "", baseUrl: null, model: null, modelOptions: null,
      authentication: null, usageQuery: null, officialQuotaRefreshIntervalMinutes: null,
      name: current.name || (official ? "" : t("providers.editor.officialDefaultName", { client: clientName(current.app) })),
    }));
    setLoginDone(false);
  };
  return <Button variant="ghost" disabled={busy} onClick={change}>{t(official ? "providerSetup.thirdParty" : "providerSetup.official")}</Button>;
}

function ClientField({ editor }: IdentityProps) {
  const { t } = useI18n();
  return <div className="asb-field"><span>{t("providers.editor.client")}</span>
    <p className="asb-provider-identity-value"><ClientLogo app={editor.draft.app} className="asb-edit-logo" />{clientName(editor.draft.app)}</p>
  </div>;
}

export function ProviderIdentityFields(props: IdentityProps) {
  const { t } = useI18n();
  const { editor, busy } = props;
  const { draft, setDraft } = editor;
  return (
    <SettingsGroup title={t("providers.editor.section.identity")}>
      <div className="asb-editor-section-fields">
        <div className="asb-provider-field-grid"><ClientField {...props} /></div>
        <div className="asb-provider-field-grid">
          <label className="asb-field"><span>{t("providers.editor.name")}</span>
            <Input value={draft.name} required disabled={busy}
              onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} />
          </label>
          <label className="asb-field"><span>{t("providers.editor.website")}</span>
            <Input type="url" value={draft.websiteUrl ?? ""} disabled={busy} placeholder={t("providers.editor.optional")}
              onChange={(event) => setDraft((current) => ({ ...current, websiteUrl: event.target.value }))} />
          </label>
        </div>
      </div>
    </SettingsGroup>
  );
}

export function ProviderNotesField({ value, busy, onChange }: {
  value: string | null;
  busy: boolean;
  onChange: (value: string) => void;
}) {
  const { t } = useI18n();
  return (
    <label className="asb-field asb-provider-advanced-group">
      <span>{t("providers.editor.notes")}</span>
      <Textarea rows={3} value={value ?? ""} disabled={busy}
        placeholder={t("providers.editor.notesPlaceholder")}
        onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
