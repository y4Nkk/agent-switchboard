import type { ConfigFileStatus, ConfigurationSources, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Select } from "./Select";
import { SettingsGroup, SettingsRow } from "./SettingsLayout";

const automatic: SettingValue = { mode: "automatic" };

export function CodexPermissionProfile({ status, report, baseline, draft, busy, inspect, onChange }: {
  status?: ConfigFileStatus;
  report: ConfigurationSources | null;
  baseline: string | null | undefined;
  draft?: SettingValue;
  busy: boolean;
  inspect: () => void;
  onChange: (value: SettingValue) => void;
}) {
  const { t } = useI18n();
  const profile = status?.codexPermissionProfile;
  const current = baseline === null || baseline === undefined ? automatic : { mode: "explicit" as const, value: baseline };
  const candidate = draft ?? current;
  const value = candidate.mode === "automatic" ? null : String(candidate.value);
  const options = report?.profileOptions.map((name) => ({ value: name, label: name })) ?? [];
  const canChoose = !busy && report?.profilesVerified && !report.profileOwnedByFragment && options.length > 0 && baseline !== undefined;
  return <SettingsGroup title={t("clientConfig.permissions.title")}>
    <SettingsRow label={t("clientConfig.permissions.defaultProfile")} details={<>
      {profile?.legacySettings.length ? <p className="asb-field-error" role="status">
        {t(profile.hasProfiles ? "clientConfig.permissions.conflict" : "clientConfig.permissions.legacySet", { keys: profile.legacySettings.join(", ") })}
      </p> : null}
      {report?.profilesVerified && options.length === 0 && <p className="asb-field-help">{t("clientConfig.permissions.noProfiles")}</p>}
      {report?.profileOwnedByFragment && <p className="asb-field-help">{t("clientConfig.permissions.fragmentOwnsProfile")}</p>}
    </>}>
      <Select value={options.some((option) => option.value === value) ? value : null} options={options}
        placeholder={value ?? t("clientConfig.settings.notSet")}
        ariaLabel={t("clientConfig.permissions.defaultProfile")} disabled={!canChoose}
        onChange={(name) => onChange({ mode: "explicit", value: name })} />
      {!report?.profilesVerified && <Button variant="secondary" onClick={inspect} disabled={busy}>{t("clientConfig.sources.title")}</Button>}
      {value !== null && <Button variant="secondary" onClick={() => onChange(automatic)}
        disabled={busy || baseline === undefined || report?.profileOwnedByFragment === true}>
        {t("clientConfig.settings.removeUserValue")}
      </Button>}
      {draft && <Button variant="secondary" onClick={() => onChange(current)} disabled={busy}>{t("clientConfig.settings.undoChange")}</Button>}
    </SettingsRow>
  </SettingsGroup>;
}
