import { RefreshCw } from "lucide-react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { ClientFilter, type ClientFilterValue } from "../ClientFilter";
import { SearchIcon } from "../icons";
import { Tooltip } from "../Tooltip";
import { SessionOrganizationFilters } from "./SessionOrganizationFilters";
import type { SessionSearch } from "./use-session-search";

export function SessionSearchControls({ search, disabled }: {
  search: SessionSearch; disabled: boolean;
}) {
  const { t } = useI18n();
  return <div className="asb-session-search-controls">
    <form className="asb-session-search-form" onSubmit={(event) => { event.preventDefault(); search.submit(); }}>
      <textarea className="asb-session-search-query" rows={1} value={search.draft} disabled={disabled}
        aria-label={t("sessions.search.aria")} placeholder={t("sessions.search.placeholder")}
        onChange={(event) => search.setDraft(event.target.value)} onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault(); search.submit();
          }
        }} />
      <Tooltip label={t("sessions.search.submit")} side="bottom">
        <Button variant="icon" size="compact" type="submit" disabled={disabled} aria-label={t("sessions.search.submit")}>
          <SearchIcon /></Button>
      </Tooltip>
    </form>
    <SessionOrganizationFilters search={search} disabled={disabled} />
  </div>;
}

export type SessionAction = "sources" | "purge" | "transfer";

export function SessionPageActions({ filter, onFilterChange, refresh, busy, refreshKey, history, onAction }: {
  filter: ClientFilterValue; onFilterChange: (value: ClientFilterValue) => void;
  refresh: () => void; busy: boolean; refreshKey: "sessions.refresh" | "sessions.bookmarks.refresh";
  history: boolean; onAction: (action: SessionAction) => void;
}) {
  const { t } = useI18n();
  return <>
    {history && filter === "claude" && <>
      <Button variant="secondary" disabled={busy} onClick={() => onAction("sources")}>{t("sessions.sources.title")}</Button>
      <Button variant="secondary" disabled={busy} onClick={() => onAction("purge")}>{t("sessions.purge.title")}</Button>
    </>}
    {history && filter === "codex" && <Button variant="secondary" disabled={busy}
      onClick={() => onAction("transfer")}>{t("sessions.transfer.title")}</Button>}
    <ClientFilter value={filter} onChange={onFilterChange} label={t("sessions.filter.aria")} showLogos />
    <Button variant="icon" disabled={busy} onClick={refresh}
      aria-label={t(refreshKey)} title={t(refreshKey)}><RefreshCw aria-hidden="true" /></Button>
  </>;
}
