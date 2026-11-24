import { useCallback, useEffect, useState } from "react";
import { searchSessions, type SessionSearchPage, type SessionSearchRequest, type SessionSearchProgress } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import type { ClientFilterValue } from "../ClientFilter";
import { createSessionTimeFilter, EMPTY_SESSION_DATES, sessionDatesInOrder, type SessionDates, type SessionTimeRange } from "./session-dates";

export const SESSION_PAGE_SIZE = 50;
const SEARCH_INPUT_DELAY_MS = 300;

export function useSessionSearch(active: boolean, archived: boolean) {
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [composing, setComposing] = useState(false);
  const [filter, setFilter] = useState<ClientFilterValue>("all");
  const [project, setProject] = useState<SessionSearchRequest["project"]>(null);
  const [tag, setTag] = useState<string | null>(null);
  const [pinnedOnly, setPinnedOnly] = useState(false);
  const [timeFilter, setTimeFilter] = useState(() => createSessionTimeFilter("all", EMPTY_SESSION_DATES));
  const { activeAfter, activeBefore } = timeFilter;
  const [projects, setProjects] = useState<SessionSearchPage["projects"]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [page, setPage] = useState(1);
  const [revision, setRevision] = useState(0);
  const identity = JSON.stringify([query, filter, project, tag, pinnedOnly, activeAfter, activeBefore, archived]);
  const [snapshot, setSnapshot] = useState<{ identity: string; page: number; value: SessionSearchPage } | null>(null);
  const current = snapshot?.identity === identity ? snapshot : null;
  const result = current?.page === page ? current.value : null;
  const total = current?.value.total ?? null;
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<SessionSearchProgress | null>(null);
  const [error, setError] = useMessageState();
  const nextQuery = draft.trim();
  const filtersActive = tag !== null || pinnedOnly || activeAfter !== null || activeBefore !== null;

  useEffect(() => {
    if (!active) { setComposing(false); return; }
    if (composing || nextQuery === query) return;
    const apply = () => { setQuery(nextQuery); setPage(1); };
    if (!nextQuery) { apply(); return; }
    const timer = window.setTimeout(apply, SEARCH_INPUT_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [active, composing, nextQuery, query]);

  useEffect(() => {
    if (!active) { setSnapshot(null); setBusy(false); setProgress(null); return; }
    const controller = new AbortController();
    setBusy(true);
    setProgress(null);
    setError(null);
    const run = async () => {
      try {
        const next = await searchSessions({ query, app: filter === "all" ? null : filter,
          offset: (page - 1) * SESSION_PAGE_SIZE, project, tag, pinnedOnly, archived, activeAfter, activeBefore }, controller.signal,
          (nextProgress) => { if (!controller.signal.aborted) setProgress(nextProgress); });
        if (controller.signal.aborted) return;
        setProjects(next.projects); setTags(next.tags);
        const lastPage = Math.max(1, Math.ceil(next.total / SESSION_PAGE_SIZE));
        setSnapshot({ identity, page, value: next });
        if (page > lastPage) setPage(lastPage);
      } catch (caught) {
        if (!controller.signal.aborted) setError(caught);
      } finally {
        if (!controller.signal.aborted) { setBusy(false); setProgress(null); }
      }
    };
    void run();
    return () => controller.abort();
  }, [active, query, filter, page, revision, project, tag, pinnedOnly, activeAfter, activeBefore, archived, identity, setError]);

  const changeFilter = (value: ClientFilterValue) => { setFilter(value); setProject(null); setTag(null); setProjects([]); setTags([]); setPage(1); };
  const changeProject = (value: SessionSearchRequest["project"]) => { setProject(value); setPage(1); };
  const openProject = (value: NonNullable<SessionSearchRequest["project"]>) => {
    setProject(value); setQuery(""); setDraft(""); setTag(null); setPinnedOnly(false); setPage(1);
    setTimeFilter(createSessionTimeFilter("all", EMPTY_SESSION_DATES));
  };
  const changeTag = (value: string | null) => { setTag(value); setPage(1); };
  const changePinned = (value: boolean) => { setPinnedOnly(value); setPage(1); };
  const changeTime = (range: SessionTimeRange, dates: SessionDates) => {
    if (!sessionDatesInOrder(dates)) return;
    setTimeFilter(createSessionTimeFilter(range, dates));
    setPage(1);
  };
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  return { draft, setDraft, setComposing, query, filter, changeFilter, filtersActive, page, setPage, result, total, busy, progress, error, refresh,
    project, changeProject, openProject, projects, tags, tag, changeTag, pinnedOnly, changePinned, timeFilter, changeTime };
}

export type SessionSearch = ReturnType<typeof useSessionSearch>;
