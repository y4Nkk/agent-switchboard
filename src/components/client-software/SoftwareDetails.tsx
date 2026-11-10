import { openUrl } from "@tauri-apps/plugin-opener";
import type { ClientSoftware } from "../../api/client-software";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { FactList, type FactListRow } from "../FactList";
import { Time } from "../Time";

export function SoftwareDetails({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  const [error, setError] = useMessageState();
  return <div className="asb-software-details">
    <div className="asb-software-detail-columns">
      <InstallationFacts item={item} />
      {item.form === "cli" && <VersionFacts item={item} />}
    </div>
    <UpdateDiagnostics item={item} />
    <Button variant="ghost" size="compact" onClick={() => void openUrl(item.guideUrl).catch(setError)}>{t("software.guide")}</Button>
    {error && <p role="alert" className="asb-field-error">{error}</p>}
  </div>;
}

function InstallationFacts({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  return <section className="asb-software-detail-section">
    <h4 className="asb-group-title">{t("software.installationInfo")}</h4>
    <FactList rows={[
      { label: t("software.installMethod"), value: t(`software.manager.${item.manager}`) },
      { label: t("software.launcher"), value: <code>{item.launcher}</code> },
      { label: t("software.executable"), value: <code>{item.executable}</code> },
      ...(item.versionText ? [{ label: t("software.versionResponse"), value: <code>{item.versionText}</code> }] : []),
    ]} />
  </section>;
}

function VersionFacts({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  const version = (value: string | null | undefined) => <code>{value ?? t("software.unknown")}</code>;
  return <section className="asb-software-detail-section">
    <h4 className="asb-group-title">{t("software.versionInfo")}</h4>
    <FactList rows={[
      { label: t("software.local"), value: version(item.version) },
      { label: t("software.official"), value: version(item.official?.version) },
      { label: t("software.available"), value: version(item.available?.version) },
      { label: t("software.channel"), value: item.channel },
      ...(item.official ? [
        { label: t("software.officialSource"), value: <code>{item.official.source}</code> },
        { label: t("software.officialCheckedAt"), value: <Time iso={item.official.checkedAt} /> },
      ] : []),
      ...(item.available ? [
        { label: t("software.channelSource"), value: <code>{item.available.source}</code> },
        { label: t("software.channelCheckedAt"), value: <Time iso={item.available.checkedAt} /> },
      ] : []),
    ]} />
  </section>;
}

function UpdateDiagnostics({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  const rows: FactListRow[] = [
    ...(item.form === "cli" ? [{ label: t("software.comparison"), value: t(`software.comparison.${item.comparison}`) }] : []),
    ...(item.reason && item.reason !== "current" && item.reason !== "ahead"
      ? [{ label: t("software.updateCondition"), value: t(`software.reason.${item.reason}`) }] : []),
    ...(item.policy.automaticDisabled && !item.policy.updatesDisabled
      ? [{ label: t("software.backgroundUpdates"), value: t("software.autoDisabled") }] : []),
    ...(item.policy.minimum ? [{ label: t("software.minimum"), value: item.policy.minimum }] : []),
    ...(item.policy.maximum ? [{ label: t("software.maximum"), value: item.policy.maximum }] : []),
    ...(item.official?.stale || item.available?.stale
      ? [{ label: t("software.freshness"), value: <span className="asb-warn-text">{t("software.stale")}</span> }] : []),
    ...(item.releaseError ? [{ label: t("software.releaseDiagnostic"), value: <code className="asb-warn-text">{item.releaseError}</code> }] : []),
    ...(item.detail ? [{ label: t("software.installationDiagnostic"), value: <code className="asb-warn-text">{item.detail}</code> }] : []),
    ...(item.policy.issue ? [{ label: t("software.policyDiagnostic"), value: <code className="asb-warn-text">{item.policy.issue}</code> }] : []),
  ];
  if (rows.length === 0) return null;
  return <section className="asb-software-detail-section">
    <h4 className="asb-group-title">{t("software.diagnostics")}</h4>
    <FactList rows={rows} />
  </section>;
}
