import { useCallback, useEffect, useState } from "react";
import { searchSessions, type SessionSearchPage, type SessionSearchRequest, type SessionSearchProgress } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import type { ClientFilterValue } from "../ClientFilter";

export const SESSION_PAGE_SIZE = 50;

export function useSessionSearch(active: boolean) {
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ClientFilterValue>("all");
  const [project, setProject] = useState<SessionSearchRequest["project"]>(null);
  const [tag, setTag] = useState<string | null>(null);
  const [pinnedOnly, setPinnedOnly] = useState(false);
  const [dates, setDates] = useState({ activeAfter: null as string | null, activeBefore: null as string | null });
  const [timeDraft, setTimeDraft] = useState({ range: "all", from: "", to: "" });
  const [projects, setProjects] = useState<SessionSearchPage["projects"]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [issues, setIssues] = useState<SessionSearchPage["issues"]>([]);
  const [page, setPage] = useState(1);
  const [revision, setRevision] = useState(0);
  const identity = JSON.stringify([query, filter, page, project, tag, pinnedOnly, dates]);
  const [snapshot, setSnapshot] = useState<{ identity: string; value: Omit<SessionSearchPage, "issues"> } | null>(null);
  const result = snapshot?.identity === identity ? snapshot.value : null;
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<SessionSearchProgress | null>(null);
  const [error, setError] = useMessageState();

  useEffect(() => {
    if (!active) { setSnapshot(null); setBusy(false); setProgress(null); return; }
    const controller = new AbortController();
    setBusy(true);
    setProgress(null);
    setError(null);
    const run = async () => {
      try {
        const next = await searchSessions({ query, app: filter === "all" ? null : filter,
          offset: (page - 1) * SESSION_PAGE_SIZE, project, tag, pinnedOnly, ...dates }, controller.signal,
          (nextProgress) => { if (!controller.signal.aborted) setProgress(nextProgress); });
        if (controller.signal.aborted) return;
        const { issues: nextIssues, ...value } = next;
        setIssues(nextIssues);
        setProjects(next.projects); setTags(next.tags);
        const lastPage = Math.max(1, Math.ceil(next.total / SESSION_PAGE_SIZE));
        if (page > lastPage) setPage(lastPage);
        else setSnapshot({ identity, value });
      } catch (caught) {
        if (!controller.signal.aborted) setError(caught);
      } finally {
        if (!controller.signal.aborted) { setBusy(false); setProgress(null); }
      }
    };
    void run();
    return () => controller.abort();
  }, [active, query, filter, page, revision, project, tag, pinnedOnly, dates, identity, setError]);

  const changeDraft = (value: string) => {
    setDraft(value);
    if (!value.trim() && query) { setQuery(""); setPage(1); }
  };
  const submit = () => { setQuery(draft.trim()); setPage(1); setRevision((value) => value + 1); };
  const changeFilter = (value: ClientFilterValue) => { setFilter(value); setProject(null); setTag(null); setProjects([]); setTags([]); setPage(1); };
  const changeProject = (value: SessionSearchRequest["project"]) => { setProject(value); setPage(1); };
  const openProject = (value: NonNullable<SessionSearchRequest["project"]>) => {
    setProject(value); setQuery(""); setDraft(""); setTag(null); setPinnedOnly(false); setPage(1);
    setDates({ activeAfter: null, activeBefore: null }); setTimeDraft({ range: "all", from: "", to: "" });
  };
  const changeTag = (value: string | null) => { setTag(value); setPage(1); };
  const changePinned = (value: boolean) => { setPinnedOnly(value); setPage(1); };
  const changeDates = (value: typeof dates) => { setDates(value); setPage(1); };
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  return { draft, setDraft: changeDraft, query, filter, changeFilter, page, setPage, result, busy, progress, issues, error, submit, refresh,
    project, changeProject, openProject, projects, tags, tag, changeTag, pinnedOnly, changePinned, changeDates, timeDraft, setTimeDraft };
}

export type SessionSearch = ReturnType<typeof useSessionSearch>;
