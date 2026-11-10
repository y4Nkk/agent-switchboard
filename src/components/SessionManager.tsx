import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Input } from "./Input";
import { Tabs } from "./Tabs";
import { WorkspaceHeader } from "./WorkspaceHeader";
import { SessionBookmarksPane } from "./session/SessionBookmarksPane";
import { SessionDeletion } from "./session/SessionDeletion";
import { SessionPageActions, SessionSearchControls } from "./session/SessionSearchControls";
import { SessionHistory } from "./session/SessionHistory";
import { SessionRangeDialog } from "./session/SessionRangeDialog";
import { SessionSourcesDialog } from "./session/SessionSourcesDialog";
import { CodexSessionProviderTransfer } from "./session/CodexSessionProviderTransfer";
import { useSessionWorkspace } from "./session/use-session-workspace";

export function SessionManager({ active }: { active: boolean }) {
  const { t } = useI18n();
  const { view, history, changeView, action, setAction, bookmarkQuery, setBookmarkQuery, bookmarkFilter, setBookmarkFilter,
    search, detail, bookmarks, deletion, organization, busy, open, refresh, changeHistoryFilter, rangeChanged } = useSessionWorkspace(active);
  return <div className="asb-sessions">
    <WorkspaceHeader title={t("nav.page.sessions")} primary={
      <Tabs value={view} onChange={changeView} scope="sessions" label={t("sessions.views")} tabs={[
        { value: "history", label: t("sessions.list.heading"), controls: "sessions-history-panel", disabled: busy },
        { value: "archived", label: t("sessions.archive.title"), controls: "sessions-history-panel", disabled: busy },
        { value: "bookmarks", label: t("sessions.bookmarks.title"), controls: "sessions-bookmarks-panel", disabled: busy },
      ]} />
    } primaryActions={<SessionPageActions filter={history ? search.filter : bookmarkFilter}
      history={history} onAction={setAction}
      onFilterChange={history ? changeHistoryFilter : setBookmarkFilter}
      refresh={history ? refresh : bookmarks.refresh}
      refreshKey={history ? "sessions.refresh" : "sessions.bookmarks.refresh"}
      busy={history ? search.busy || busy : bookmarks.loading || bookmarks.busy} />}
      secondary={history ? <SessionSearchControls search={search} disabled={busy} />
        : <div className="asb-session-bookmark-search"><Input type="search" value={bookmarkQuery}
          onChange={(event) => setBookmarkQuery(event.target.value)} aria-label={t("sessions.bookmarks.search")}
          placeholder={t("sessions.bookmarks.search")} /></div>} />
    {action === "sources" && <SessionSourcesDialog onClose={() => setAction(null)} onChanged={refresh} />}
    {action === "range" && <SessionRangeDialog app={search.filter === "all" ? null : search.filter}
      archived={view === "archived"} dates={search.timeDraft} onClose={() => setAction(null)} onChanged={rangeChanged} />}
    {action === "transfer" && <CodexSessionProviderTransfer onClose={() => setAction(null)} onChanged={refresh} />}
    {bookmarks.error && <div className="asb-session-bookmark-error" role="alert">
      <p className="asb-warn-text">{bookmarks.error}</p>
      <Button variant="secondary" disabled={bookmarks.loading || bookmarks.busy}
        onClick={bookmarks.refresh}>{t("sessions.bookmarks.refresh")}</Button>
    </div>}
    {history && organization.error && <p className="asb-warn-text" role="alert">{organization.error}</p>}
    {history && organization.status && <p className="asb-scope-note" role="status">{organization.status}</p>}
    {history && <SessionDeletion state={deletion} disabled={organization.busy} />}
    <div className="asb-session-panel" id="sessions-history-panel" role="tabpanel" aria-labelledby={view === "archived" ? "sessions-archived-tab" : "sessions-history-tab"} hidden={!history}>
      <SessionHistory search={search} detail={detail} bookmarks={bookmarks} organization={organization}
        deletion={deletion} onOpen={open} />
    </div>
    <div className="asb-session-panel" id="sessions-bookmarks-panel" role="tabpanel" aria-labelledby="sessions-bookmarks-tab" hidden={view !== "bookmarks"}>
      <SessionBookmarksPane bookmarks={bookmarks} onOpen={open} />
    </div>
  </div>;
}
