import { useId, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ClientSoftware } from "../../api/client-software";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { Table, type TableColumn } from "../Table";
import { SoftwareDetails } from "./SoftwareDetails";

export function SoftwareTable({ items, disabled, onUpdate }: {
  items: ClientSoftware[]; disabled: boolean; onUpdate: (id: string) => void;
}) {
  const { t } = useI18n();
  const detailsId = useId();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [error, setError] = useMessageState();
  const version = (value: string | null | undefined) => <code>{value ?? t("software.unknown")}</code>;
  const columns: TableColumn<ClientSoftware>[] = [
    { key: "installation", header: t("software.installation"), render: (item) => <div className="asb-software-identity">
      <strong>{t(`software.form.${item.form}`)}</strong>
      <span className="asb-scope-note">{t(item.active ? "software.active" : "software.other")}</span>
      <span className="asb-scope-note">{t(`software.manager.${item.manager}`)}</span>
    </div> },
    { key: "local", header: t("software.local"), render: (item) => version(item.version) },
    { key: "official", header: t("software.official"), render: (item) => item.form === "cli" ? version(item.official?.version) : "—" },
    { key: "available", header: t("software.available"), render: (item) => item.form === "cli" ? version(item.available?.version) : "—" },
    { key: "channel", header: t("software.channel"), render: (item) => item.form === "cli" ? item.channel : "—" },
    { key: "status", header: t("software.status"), render: (item) => <SoftwareStatus item={item} /> },
    { key: "actions", header: t("software.actions"), render: (item) => <div className="asb-software-actions">
      {item.updateAllowed && <Button variant="primary" size="compact" disabled={disabled}
        onClick={() => onUpdate(item.id)}>{t("software.update")}</Button>}
      <Button variant="ghost" size="compact" onClick={() => void openUrl(item.guideUrl).catch(setError)}>{t("software.guide")}</Button>
      <CollapsibleTrigger open={expanded === item.id} contentId={`${detailsId}-${item.id}`} label={t("software.details")}
        onOpenChange={(open) => setExpanded(open ? item.id : null)} />
    </div> },
  ];
  return <>
    <Table columns={columns} rows={items} rowKey={(item) => item.id} ariaLabel={t("software.installations")}
      className="asb-software-table" renderExpansion={(item) => <Collapsible
        open={expanded === item.id} id={`${detailsId}-${item.id}`}>
        <SoftwareDetails item={item} />
      </Collapsible>} />
    {error && <p role="alert" className="asb-field-error">{error}</p>}
  </>;
}

function SoftwareStatus({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  return <div className="asb-software-status">
    {item.form === "cli" && <p className="asb-software-state">{t(`software.comparison.${item.comparison}`)}</p>}
    {item.reason && <p className="asb-scope-note">{t(`software.reason.${item.reason}`)}</p>}
    {item.policy.automaticDisabled && !item.policy.updatesDisabled && <p className="asb-scope-note">{t("software.autoDisabled")}</p>}
    {item.releaseError && <p className="asb-warn-text">{item.releaseError}</p>}
  </div>;
}
