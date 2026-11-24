import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Input } from "./Input";
import { OverlayMotionContext } from "./overlay-motion";
import { Tabs } from "./Tabs";
import { WorkspaceHeader } from "./WorkspaceHeader";
import { SessionBatchDialog } from "./session/SessionBatchDialog";
import { SessionBookmarksPane } from "./session/SessionBookmarksPane";
import { SessionDeletionConfirm } from "./session/SessionDeletion";
import { SessionPageActions } from "./session/SessionPageActions";
import { SessionSearchControls } from "./session/SessionSearchControls";
import { SessionHistory } from "./session/SessionHistory";
import { SessionOrganizationFilters } from "./session/SessionOrganizationFilters";
import { SessionRangeDialog } from "./session/SessionRangeDialog";
import { SessionSourcesDialog } from "./session/SessionSourcesDialog";
import { CodexSessionProviderTransfer } from "./session/CodexSessionProviderTransfer";
import { useOnReturnToRoot } from "../app/PageRoot";
import { useSessionWorkspace } from "./session/use-session-workspace";

export function SessionManager({ active }: { active: boolean }) {
  const { t } = useI18n();
  const { view, history, changeView, clearSelection, action, openAction, closeAction, batchOpen, filtersOpen,
    bookmarkQuery, setBookmarkQuery, bookmarkFilter, setBookmarkFilter,
    search, detail, bookmarks, selection, deletion, organization, busy, open, refresh, changeHistoryFilter, rangeChanged } = useSessionWorkspace(active);
  useOnReturnToRoot(active && (detail.selected !== null || batchOpen), clearSelection, busy);
  return <OverlayMotionContext.Provider value><div className="asb-sessions">
    <WorkspaceHeader title={t("nav.page.sessions")} primary={
      <Tabs value={view} onChange={changeView} scope="sessions" label={t("sessions.views")} tabs={[
        { value: "history", label: t("sessions.list.heading"), controls: "sessions-history-panel", disabled: busy },
        { value: "archived", label: t("sessions.archive.title"), controls: "sessions-history-panel", disabled: busy },
        { value: "bookmarks", label: t("sessions.bookmarks.title"), controls: "sessions-bookmarks-panel", disabled: busy },
      ]} />
    } primaryActions={<SessionPageActions filter={history ? search.filter : bookmarkFilter}
      history={history} onAction={openAction}
      onFilterChange={history ? changeHistoryFilter : setBookmarkFilter}
      refresh={history ? refresh : bookmarks.refresh}
      refreshKey={history ? "sessions.refresh" : "sessions.bookmarks.refresh"}
      busy={history ? busy : bookmarks.busy} loading={history ? search.busy : bookmarks.loading} />}
      secondary={history ? <SessionSearchControls search={search} disabled={busy}
        filtersOpen={filtersOpen} onOpenFilters={() => openAction("filters")} />
        : <div className="asb-session-bookmark-search"><Input type="search" value={bookmarkQuery}
          onChange={(event) => setBookmarkQuery(event.target.value)} aria-label={t("sessions.bookmarks.search")}
          placeholder={t("sessions.bookmarks.search")} /></div>} />
    {action === "sources" && <SessionSourcesDialog onClose={closeAction} onChanged={refresh} />}
    {action === "range" && <SessionRangeDialog app={search.filter === "all" ? null : search.filter}
      archived={view === "archived"} dates={search.timeFilter.dates} onClose={closeAction} onChanged={rangeChanged} />}
    {action === "transfer" && <CodexSessionProviderTransfer onClose={closeAction} onChanged={refresh} />}
    {filtersOpen && <SessionOrganizationFilters search={search} busy={busy} onClose={closeAction} />}
    {bookmarks.error && <div className="asb-session-bookmark-error" role="alert">
      <p className="asb-warn-text">{bookmarks.error}</p>
      <Button variant="secondary" disabled={bookmarks.loading || bookmarks.busy}
        onClick={bookmarks.refresh}>{t("sessions.bookmarks.refresh")}</Button>
    </div>}
    {history && !batchOpen && <>
      {organization.error && <p className="asb-warn-text" role="alert">{organization.error}</p>}
      {organization.status && <p className="asb-scope-note" role="status">{organization.status}</p>}
      {deletion.error && <p className="asb-warn-text" role="alert">{deletion.error}</p>}
    </>}
    {batchOpen && <SessionBatchDialog search={search} selection={selection} deletion={deletion} organization={organization}
      onClose={closeAction} />}
    {history && !batchOpen && (active || deletion.busy) && <SessionDeletionConfirm state={deletion} disabled={organization.busy} />}
    <div className="asb-session-panel" id="sessions-history-panel" role="tabpanel" data-view={view}
      aria-labelledby={view === "archived" ? "sessions-archived-tab" : "sessions-history-tab"} hidden={!history}>
      <SessionHistory search={search} detail={detail} bookmarks={bookmarks} organization={organization}
        deletion={deletion} onOpen={open} />
    </div>
    <div className="asb-session-panel" id="sessions-bookmarks-panel" role="tabpanel" aria-labelledby="sessions-bookmarks-tab" hidden={view !== "bookmarks"}>
      <SessionBookmarksPane bookmarks={bookmarks} onOpen={open} />
    </div>
  </div></OverlayMotionContext.Provider>;
}
