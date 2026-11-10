import { useState } from "react";
import type { SessionMeta } from "../../api/client";
import { useWorkspaceVisible } from "../../hooks/use-workspace-visible";
import type { ClientFilterValue } from "../ClientFilter";
import type { SessionAction } from "./SessionSearchControls";
import { useSessionDeletion } from "./SessionDeletion";
import { useSessionOrganization } from "./use-session-organization";
import { useSessionBookmarks } from "./use-session-bookmarks";
import { useSessionDetail } from "./use-session-detail";
import { useSessionSearch } from "./use-session-search";
import { sessionKey } from "./session-content";

export function useSessionWorkspace(active: boolean) {
  const [view, setView] = useState<"history" | "archived" | "bookmarks">("history");
  const [action, setAction] = useState<SessionAction | null>(null);
  const [bookmarkQuery, setBookmarkQuery] = useState("");
  const [bookmarkFilter, setBookmarkFilter] = useState<ClientFilterValue>("all");
  const current = useWorkspaceVisible(active);
  const history = view !== "bookmarks";
  const search = useSessionSearch(current && history, view === "archived");
  const detail = useSessionDetail(current && history);
  const bookmarks = useSessionBookmarks(current && !history, bookmarkQuery, bookmarkFilter);
  const deletion = useSessionDeletion((removed) => {
    if (detail.selected && removed.has(sessionKey(detail.selected))) detail.clear();
    search.refresh();
  });
  const organization = useSessionOrganization((updated) => {
    detail.updateMetadata(updated); deletion.updateMetadata(updated); search.refresh();
  });
  const busy = deletion.busy || organization.busy;
  const clearSelection = () => {
    detail.clear(); deletion.setPending([]);
    if (deletion.selecting) deletion.toggleMode();
  };
  const open = (session: SessionMeta, messageId: string | null = null, contentOffset = 0, archived = view === "archived") => {
    if (search.filter !== "all" && search.filter !== session.app) search.changeFilter(session.app);
    setView(archived ? "archived" : "history");
    detail.select(session, messageId, contentOffset);
  };
  const refresh = () => { search.refresh(); bookmarks.refresh(); detail.refresh(); };
  const changeView = (next: typeof view) => {
    if (next === view) return;
    clearSelection(); search.setPage(1); setView(next);
  };
  const changeHistoryFilter = (next: ClientFilterValue) => {
    if (next === search.filter) return;
    clearSelection(); search.changeFilter(next);
  };
  const rangeChanged = () => { clearSelection(); refresh(); };
  return { view, history, changeView, action, setAction, bookmarkQuery, setBookmarkQuery, bookmarkFilter, setBookmarkFilter,
    search, detail, bookmarks, deletion, organization, busy, open, refresh, changeHistoryFilter, rangeChanged };
}
