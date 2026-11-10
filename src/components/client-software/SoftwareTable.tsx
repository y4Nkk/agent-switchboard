import { useId, useState } from "react";
import type { ClientSoftware } from "../../api/client-software";
import { useI18n } from "../../i18n";
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
  const version = (value: string | null | undefined) => <code>{value ?? "—"}</code>;
  const columns: TableColumn<ClientSoftware>[] = [
    { key: "installation", header: t("software.installation"), render: (item) => <strong>{t(`software.form.${item.form}`)}</strong> },
    { key: "usage", header: t("software.usage"), render: (item) => t(item.active ? "software.active" : "software.other") },
    { key: "local", header: t("software.local"), render: (item) => version(item.version) },
    { key: "available", header: t("software.channelVersion"), render: (item) => item.form === "cli" ? version(item.available?.version) : "—" },
    { key: "status", header: t("software.status"), render: (item) => <SoftwareStatus item={item} /> },
    { key: "actions", header: t("software.actions"), render: (item) => <div className="asb-software-actions">
      {item.updateAllowed && <Button variant="primary" size="compact" disabled={disabled}
        onClick={() => onUpdate(item.id)}>{t("software.update")}</Button>}
      <CollapsibleTrigger open={expanded === item.id} contentId={`${detailsId}-${item.id}`} label={t("software.detailsAction")}
        onOpenChange={(open) => setExpanded(open ? item.id : null)} />
    </div> },
  ];
  return <Table columns={columns} rows={items} rowKey={(item) => item.id} ariaLabel={t("software.installations")}
    className="asb-software-table" renderExpansion={(item) => <Collapsible
      open={expanded === item.id} id={`${detailsId}-${item.id}`}>
      <SoftwareDetails item={item} />
    </Collapsible>} />;
}

function SoftwareStatus({ item }: { item: ClientSoftware }) {
  const { t } = useI18n();
  let label: string;
  if (item.form !== "cli") label = t("software.summary.hostManaged");
  else if (item.releaseError) label = t("software.summary.checkFailed");
  else if (item.reason && item.reason !== "current" && item.reason !== "ahead") label = t(`software.summary.${item.reason}`);
  else label = t(`software.summary.${item.comparison}`);
  return <span className={item.releaseError || item.policy.issue ? "asb-warn-text" : undefined}>{label}</span>;
}
