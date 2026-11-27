import { useI18n } from "../../i18n";
import { ClientLogo } from "../ClientLogo";
import { Input } from "../Input";
import { SettingsGroup } from "../SettingsLayout";
import { Button } from "../Button";
import type { CodexEditorDraft } from "./draft";
import type { SetCodexDraft } from "./useCodexProviderEditor";

interface IdentityProps {
  draft: CodexEditorDraft;
  busy: boolean;
  setDraft: SetCodexDraft;
}

interface AccessModeProps {
  busy: boolean;
  onSwitchAccessMode: (official: boolean) => void;
}

export function CodexAccessMode({ busy, onSwitchAccessMode }: AccessModeProps) {
  const { t } = useI18n();
  return <Button variant="ghost" disabled={busy} onClick={() => onSwitchAccessMode(true)}>{t("providerSetup.official")}</Button>;
}

function ClientField() {
  const { t } = useI18n();
  return <div className="asb-field"><span>{t("codex.identity.client")}</span>
    <p className="asb-provider-identity-value"><ClientLogo app="codex" className="asb-edit-logo" />Codex</p>
  </div>;
}

export function CodexIdentityFields(props: IdentityProps) {
  const { t } = useI18n();
  const { draft, busy, setDraft } = props;
  return (
    <SettingsGroup title={t("codex.identity.title")}>
      <div className="asb-editor-section-fields">
        <div className="asb-provider-field-grid"><ClientField /></div>
        <div className="asb-provider-field-grid">
          <label className="asb-field"><span>{t("codex.identity.name")}</span>
            <Input value={draft.name} required disabled={busy}
              onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} />
          </label>
          <label className="asb-field"><span>{t("codex.identity.website")}</span>
            <Input type="url" value={draft.websiteUrl} disabled={busy} placeholder={t("codex.optional")}
              onChange={(event) => setDraft((current) => ({ ...current, websiteUrl: event.target.value }))} />
          </label>
        </div>
      </div>
    </SettingsGroup>
  );
}
