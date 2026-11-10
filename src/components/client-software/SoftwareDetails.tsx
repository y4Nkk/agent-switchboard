import type { ClientSoftware } from "../../api/client-software";
import { useI18n } from "../../i18n";
import { FactList } from "../FactList";
import { Time } from "../Time";

export function SoftwareDetails({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  return <div className="asb-software-details">
    <FactList rows={[
      { label: t("software.launcher"), value: <code>{item.launcher}</code> },
      { label: t("software.executable"), value: <code>{item.executable}</code> },
      ...(item.versionText ? [{ label: t("software.versionResponse"), value: <code>{item.versionText}</code> }] : []),
      ...(item.official ? [
        { label: t("software.source"), value: <code>{item.official.source}</code> },
        { label: t("software.checkedAt"), value: <Time iso={item.official.checkedAt} /> },
      ] : []),
      ...(item.available && item.available.source !== item.official?.source ? [
        { label: t("software.available"), value: <code>{item.available.source}</code> },
        { label: t("software.checkedAt"), value: <Time iso={item.available.checkedAt} /> },
      ] : []),
      ...(item.policy.minimum ? [{ label: t("software.minimum"), value: item.policy.minimum }] : []),
      ...(item.policy.maximum ? [{ label: t("software.maximum"), value: item.policy.maximum }] : []),
    ]} />
    {(item.official?.stale || item.available?.stale) && <p className="asb-warn-text">{t("software.stale")}</p>}
    {item.detail && <p className="asb-warn-text">{item.detail}</p>}
    {item.policy.issue && <p className="asb-warn-text">{item.policy.issue}</p>}
  </div>;
}
