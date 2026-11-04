import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ClientSoftware } from "../../api/client-software";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { CollapsibleSection } from "../Collapsible";
import { FactList } from "../FactList";
import { Time } from "../Time";

export function SoftwareCard({ item, disabled, onUpdate }: {
  item: ClientSoftware; disabled: boolean; onUpdate: () => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [error, setError] = useMessageState();
  const version = (value: string | null | undefined) => value ?? t("software.unknown");
  const cli = item.form === "cli";
  return (
    <article className="asb-software-installation">
      <div className="asb-software-heading">
        <strong>{t(`software.form.${item.form}`)}</strong>
        <span className="asb-scope-note">{t(item.active ? "software.active" : "software.other")}</span>
        <span className="asb-scope-note">{t(`software.manager.${item.manager}`)}</span>
      </div>
      <FactList rows={[
        { label: t("software.local"), value: <code>{version(item.version)}</code> },
        ...(cli ? [
          { label: t("software.official"), value: <code>{version(item.official?.version)}</code> },
          { label: t("software.available"), value: <code>{version(item.available?.version)}</code> },
          { label: t("software.channel"), value: item.channel },
        ] : []),
      ]} />
      {cli && <p className="asb-software-state">{t(`software.comparison.${item.comparison}`)}</p>}
      {item.reason && <p className="asb-scope-note">{t(`software.reason.${item.reason}`)}</p>}
      {item.policy.automaticDisabled && !item.policy.updatesDisabled && <p className="asb-scope-note">{t("software.autoDisabled")}</p>}
      {item.releaseError && <p className="asb-warn-text">{item.releaseError}</p>}
      <div className="asb-software-actions">
        {item.updateAllowed && <Button variant="primary" size="compact" disabled={disabled} onClick={onUpdate}>{t("software.update")}</Button>}
        <Button variant="ghost" size="compact" onClick={() => void openUrl(item.guideUrl).catch(setError)}>{t("software.guide")}</Button>
      </div>
      {error && <p role="alert" className="asb-field-error">{error}</p>}
      <CollapsibleSection quiet open={open} onOpenChange={setOpen} label={t("software.details")}>
        <SoftwareDetails item={item} />
      </CollapsibleSection>
    </article>
  );
}

function SoftwareDetails({ item }: { item: ClientSoftware }) {
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
