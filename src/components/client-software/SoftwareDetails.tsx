import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowDownToLine, Clock3, Link2, Package, ShieldCheck, Tag, Terminal, TriangleAlert } from "lucide-react";
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
      { label: t("software.installMethod"), icon: Package, value: t(`software.manager.${item.manager}`) },
      { label: t("software.launcher"), icon: Terminal, format: "code", value: item.launcher },
      { label: t("software.executable"), icon: Terminal, format: "code", value: item.executable },
      ...(item.versionText ? [{ label: t("software.versionResponse"), icon: Tag,
        format: "code" as const, value: item.versionText }] : []),
    ]} />
  </section>;
}

function VersionFacts({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  const version = (value: string | null | undefined) => value ?? t("software.unknown");
  return <section className="asb-software-detail-section">
    <h4 className="asb-group-title">{t("software.versionInfo")}</h4>
    <FactList rows={[
      { label: t("software.local"), icon: Tag, format: "code", value: version(item.version) },
      { label: t("software.official"), icon: ShieldCheck, format: "code", value: version(item.official?.version) },
      { label: t("software.available"), icon: ArrowDownToLine, format: "code", value: version(item.available?.version) },
      { label: t("software.channel"), icon: Package, value: item.channel },
      ...(item.official ? [
        { label: t("software.officialSource"), icon: Link2, format: "code" as const, value: item.official.source },
        { label: t("software.officialCheckedAt"), icon: Clock3, value: <Time iso={item.official.checkedAt} /> },
      ] : []),
      ...(item.available ? [
        { label: t("software.channelSource"), icon: Link2, format: "code" as const, value: item.available.source },
        { label: t("software.channelCheckedAt"), icon: Clock3, value: <Time iso={item.available.checkedAt} /> },
      ] : []),
    ]} />
  </section>;
}

function UpdateDiagnostics({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  const rows: FactListRow[] = [
    ...(item.form === "cli" ? [{ label: t("software.comparison"), icon: Tag,
      value: t(`software.comparison.${item.comparison}`) }] : []),
    ...(item.reason && item.reason !== "current" && item.reason !== "ahead"
      ? [{ label: t("software.updateCondition"), icon: ShieldCheck, value: t(`software.reason.${item.reason}`) }] : []),
    ...(item.policy.automaticDisabled && !item.policy.updatesDisabled
      ? [{ label: t("software.backgroundUpdates"), icon: Clock3, value: t("software.autoDisabled") }] : []),
    ...(item.policy.minimum ? [{ label: t("software.minimum"), icon: Tag,
      format: "code" as const, value: item.policy.minimum }] : []),
    ...(item.policy.maximum ? [{ label: t("software.maximum"), icon: Tag,
      format: "code" as const, value: item.policy.maximum }] : []),
    ...(item.official?.stale || item.available?.stale
      ? [{ label: t("software.freshness"), icon: Clock3, tone: "warning" as const,
        value: <span className="asb-warn-text">{t("software.stale")}</span> }] : []),
    ...(item.releaseError ? [{ label: t("software.releaseDiagnostic"), icon: TriangleAlert, tone: "warning" as const,
      value: <code className="asb-warn-text">{item.releaseError}</code> }] : []),
    ...(item.detail ? [{ label: t("software.installationDiagnostic"), icon: TriangleAlert, tone: "warning" as const,
      value: <code className="asb-warn-text">{item.detail}</code> }] : []),
    ...(item.policy.issue ? [{ label: t("software.policyDiagnostic"), icon: TriangleAlert, tone: "warning" as const,
      value: <code className="asb-warn-text">{item.policy.issue}</code> }] : []),
  ];
  if (rows.length === 0) return null;
  return <section className="asb-software-detail-section">
    <h4 className="asb-group-title">{t("software.diagnostics")}</h4>
    <FactList rows={rows} />
  </section>;
}
