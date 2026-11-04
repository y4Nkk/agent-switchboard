import type { SettingsValues, SettingValue } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { SettingsFields } from "../SettingsFields";
import type { useProviderParameters } from "./useProviderParameters";
import { ClaudeModelPreferencesEditor } from "./ClaudeModelPreferencesEditor";
import { withoutModelPreferences } from "./claude-model-preferences";

interface Props {
  value: SettingsValues | null;
  onChange: (value: SettingsValues) => void;
  parameters: ReturnType<typeof useProviderParameters>;
  busy: boolean;
  baselineValues?: Record<string, SettingValue>;
  claudeFragment?: Record<string, unknown>;
  onFragmentChange?: (fragment: Record<string, unknown>) => void;
}

export function ParametersLoadStatus({ ready, error, retry, busy }: {
  ready: boolean;
  error: string | null;
  retry: () => void;
  busy: boolean;
}) {
  const { t } = useI18n();
  if (ready) return null;
  return error ? (
    <div className="asb-field-error" role="alert">
      <p>{t("providers.parameters.loadFailed", { detail: error })}</p>
      <Button variant="secondary" disabled={busy} onClick={retry}>{t("providers.parameters.retry")}</Button>
    </div>
  ) : <p className="asb-field-help" role="status">{t("providers.parameters.loading")}</p>;
}

export function ProviderParametersPage({ value, onChange, parameters, busy, baselineValues, claudeFragment, onFragmentChange }: Props) {
  const { t } = useI18n();
  const catalog = parameters.catalog;
  if (!catalog || !value) return <ParametersLoadStatus busy={busy}
    ready={parameters.ready} error={parameters.error} retry={parameters.retry} />;
  const baseline = baselineValues ?? catalog.defaults.settings;
  const resetAll = () => {
    onChange({ settings: { ...catalog.defaults.settings } });
    if (catalog.app === "claude") onFragmentChange?.(withoutModelPreferences(claudeFragment ?? {}));
  };
  const change = (key: string, next: SettingValue) =>
    onChange({ settings: { ...value.settings, [key]: next } });
  return (
    <div className="asb-form asb-provider-parameters" aria-label={t("providers.parameters.aria")}>
      <p className="asb-field-help">{t("providers.parameters.help")}</p>
      <SettingsFields specs={catalog.specs} groups={catalog.groups} values={value.settings}
        baselineValues={baseline} busy={busy} showGroupReset={false} presentation="provider" onChange={change} />
      {catalog.app === "claude" && catalog.claudeModelPreferences && onFragmentChange && <ClaudeModelPreferencesEditor fragment={claudeFragment ?? {}}
        catalog={catalog.claudeModelPreferences} busy={busy} onChange={onFragmentChange} />}
      <div className="asb-settings-actions">
        <Button variant="secondary" disabled={busy} onClick={resetAll}>{t("providers.parameters.reset")}</Button>
      </div>
    </div>
  );
}
