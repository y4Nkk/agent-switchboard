import { useCallback, useEffect, useRef, useState } from "react";
import { createSessionSearch, type SessionSearchPage, type SessionSearchRequest, type SessionSearchProgress } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import type { ClientFilterValue } from "../ClientFilter";
import { createSessionTimeFilter, EMPTY_SESSION_DATES, sessionDatesInOrder, type SessionDates, type SessionTimeRange } from "./session-dates";

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
  const [requestedPage, setPage] = useState(1);
  const [revision, setRevision] = useState(0);
  const reader = useRef<ReturnType<typeof createSessionSearch> | null>(null);
  const identity = JSON.stringify([query, filter, project, tag, pinnedOnly, activeAfter, activeBefore, archived, revision]);
  const [snapshot, setSnapshot] = useState<{ identity: string; requestedPage: number; value: SessionSearchPage } | null>(null);
  const current = snapshot?.identity === identity ? snapshot : null;
  const result = current?.requestedPage === requestedPage ? current.value : null;
  const page = result?.page ?? requestedPage;
  const pagination = current && { page, pageSize: current.value.pageSize, total: current.value.total };
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
    if (!active) return;
    const search = createSessionSearch({ query, app: filter === "all" ? null : filter,
      project, tag, pinnedOnly, archived, activeAfter, activeBefore });
    reader.current = search;
    return () => { reader.current = null; search.close(); };
  }, [active, query, filter, revision, project, tag, pinnedOnly, activeAfter, activeBefore, archived]);

  useEffect(() => {
    if (!active) { setSnapshot(null); setBusy(false); setProgress(null); return; }
    const search = reader.current;
    if (!search) return;
    const controller = new AbortController();
    setBusy(true);
    setProgress(null);
    setError(null);
    const run = async () => {
      try {
        const next = await search.page(requestedPage, controller.signal,
          (nextProgress) => { if (!controller.signal.aborted) setProgress(nextProgress); });
        if (controller.signal.aborted) return;
        setProjects(next.projects); setTags(next.tags);
        setSnapshot({ identity, requestedPage, value: next });
      } catch (caught) {
        if (!controller.signal.aborted) setError(caught);
      } finally {
        if (!controller.signal.aborted) { setBusy(false); setProgress(null); }
      }
    };
    void run();
    return () => controller.abort();
  }, [active, query, filter, requestedPage, revision, project, tag, pinnedOnly, activeAfter, activeBefore, archived, identity, setError]);

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
  const refresh = useCallback(() => {
    if (page !== requestedPage) setPage((current) => current === requestedPage ? page : current);
    setRevision((value) => value + 1);
  }, [page, requestedPage]);
  return { draft, setDraft, setComposing, query, filter, changeFilter, filtersActive, setPage, result, pagination, busy, progress, error, refresh,
    project, changeProject, openProject, projects, tags, tag, changeTag, pinnedOnly, changePinned, timeFilter, changeTime };
}

export type SessionSearch = ReturnType<typeof useSessionSearch>;
