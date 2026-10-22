import { useCallback, useEffect, useState } from "react";
import { searchSessions, type SessionSearchPage, type SessionSearchRequest } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import type { ClientFilterValue } from "../ClientFilter";
import { useReadQueue } from "../../hooks/use-read-queue";

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
  const [page, setPage] = useState(1);
  const [revision, setRevision] = useState(0);
  const identity = JSON.stringify([query, filter, page, project, tag, pinnedOnly, dates]);
  const [snapshot, setSnapshot] = useState<{ identity: string; value: SessionSearchPage } | null>(null);
  const result = snapshot?.identity === identity ? snapshot.value : null;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const enqueue = useReadQueue();

  useEffect(() => {
    if (!active) { setSnapshot(null); setBusy(false); return; }
    let current = true;
    setBusy(true);
    setError(null);
    const run = async () => {
      if (!current) return;
      try {
        const next = await searchSessions({ query, app: filter === "all" ? null : filter,
          offset: (page - 1) * SESSION_PAGE_SIZE, project, tag, pinnedOnly, ...dates });
        if (!current) return;
        setProjects(next.projects); setTags(next.tags);
        const lastPage = Math.max(1, Math.ceil(next.total / SESSION_PAGE_SIZE));
        if (page > lastPage) setPage(lastPage);
        else setSnapshot({ identity, value: next });
      } finally {
        if (current) setBusy(false);
      }
    };
    const cancel = enqueue(run, (caught) => { if (current) setError(caught); });
    return () => { current = false; cancel(); };
  }, [active, query, filter, page, revision, project, tag, pinnedOnly, dates, identity, setError, enqueue]);

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
  return { draft, setDraft, query, filter, changeFilter, page, setPage, result, busy, error, submit, refresh,
    project, changeProject, openProject, projects, tags, tag, changeTag, pinnedOnly, changePinned, changeDates, timeDraft, setTimeDraft };
}

export type SessionSearch = ReturnType<typeof useSessionSearch>;
