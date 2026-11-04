import type { ConfigFileStatus } from "../api/client";
import { useI18n } from "../i18n";

export function CodexPermissionProfileNotice({ status }: { status: ConfigFileStatus | undefined }) {
  const { t } = useI18n();
  const profile = status?.codexPermissionProfile;
  if (!profile?.hasProfiles) return null;
  return <section className="asb-toggle-group" aria-label={t("clientConfig.permissions.title")}>
    <h3 className="asb-section-title">{t("clientConfig.permissions.title")}</h3>
    <p className="asb-field-help">{profile.selected
      ? t("clientConfig.permissions.selected", { name: profile.selected })
      : t("clientConfig.permissions.defined")}</p>
    <p className="asb-field-help">{t("clientConfig.permissions.help")}</p>
    {profile.legacySettings.length > 0 && <p className="asb-field-error" role="status">
      {t("clientConfig.permissions.conflict", { keys: profile.legacySettings.join(", ") })}
    </p>}
  </section>;
}
