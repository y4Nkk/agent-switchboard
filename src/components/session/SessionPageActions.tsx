import { Ellipsis, RefreshCw } from "lucide-react";
import { useI18n } from "../../i18n";
import { ActionMenu } from "../ActionMenu";
import { Button } from "../Button";
import { ClientFilter, type ClientFilterValue } from "../ClientFilter";
import { Tooltip } from "../Tooltip";
import type { SessionAction } from "./use-session-workspace";

const CLIENT_SESSION_ACTIONS = {
  codex: { id: "transfer", label: "sessions.transfer.title" },
  claude: { id: "sources", label: "sessions.sources.title" },
} as const;

export function SessionPageActions({ filter, onFilterChange, refresh, busy, loading, refreshKey, history, onAction }: {
  filter: ClientFilterValue; onFilterChange: (value: ClientFilterValue) => void;
  refresh: () => void; busy: boolean; loading: boolean; refreshKey: "sessions.refresh" | "sessions.bookmarks.refresh";
  history: boolean; onAction: (action: SessionAction) => void;
}) {
  const { t } = useI18n();
  const moreLabel = t("sessions.actions.pageMore");
  const refreshLabel = t(refreshKey);
  const items = [
    { id: "batch", label: t("sessions.batch.title"), onAction: () => onAction("batch") },
    { id: "range", label: t("sessions.range.title"), disabled: loading, onAction: () => onAction("range") },
  ];
  if (filter !== "all") {
    const clientAction = CLIENT_SESSION_ACTIONS[filter];
    items.push({ id: clientAction.id, label: t(clientAction.label), disabled: loading,
      onAction: () => onAction(clientAction.id) });
  }
  return <>
    <ClientFilter value={filter} onChange={onFilterChange} label={t("sessions.filter.aria")} showLogos />
    {history && <ActionMenu key={filter} ariaLabel={moreLabel} disabled={busy} items={items}
      label={<><Ellipsis size={16} strokeWidth={1.7} aria-hidden="true" />{moreLabel}</>} />}
    <Tooltip label={refreshLabel} side="bottom">
      <Button variant="icon" className="asb-session-toolbar-button" disabled={busy || loading} onClick={refresh}
        aria-label={refreshLabel}><RefreshCw aria-hidden="true" /></Button>
    </Tooltip>
  </>;
}
