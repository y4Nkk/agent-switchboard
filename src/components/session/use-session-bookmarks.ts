import { useCallback, useEffect, useRef, useState } from "react";
import { deleteSessionBookmark, searchSessionBookmarks, saveSessionBookmark,
  type SessionBookmarkPage, type SessionMeta } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import type { ClientFilterValue } from "../ClientFilter";
import { toast, toastMessage } from "../use-toast";
import { useReadQueue } from "../../hooks/use-read-queue";

export function useSessionBookmarks(active: boolean, query: string, filter: ClientFilterValue) {
  const [result, setResult] = useState<SessionBookmarkPage | null>(null);
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useMessageState();
  const [revision, setRevision] = useState(0);
  const generation = useRef(0);
  const locked = useRef(false);
  const enqueue = useReadQueue();
  useEffect(() => setPage(1), [query, filter]);

  useEffect(() => {
    setResult(null);
    if (!active) { setLoading(false); return; }
    const version = ++generation.current;
    let current = true;
    setLoading(true);
    setError(null);
    const run = async () => {
      if (!current) return;
      try {
        const next = await searchSessionBookmarks(query, filter === "all" ? null : filter, (page - 1) * 50);
        if (!current || generation.current !== version) return;
        const lastPage = Math.max(1, Math.ceil(next.total / next.pageSize));
        if (page > lastPage) setPage(lastPage); else setResult(next);
      } finally { if (current) setLoading(false); }
    };
    const cancel = enqueue(run, (caught) => { if (current) setError(caught); });
    return () => { current = false; generation.current += 1; cancel(); };
  }, [active, revision, query, filter, page, setError, enqueue]);

  const save = async (session: SessionMeta, messageId: string) => {
    if (locked.current) return false;
    locked.current = true;
    setBusy(true);
    setError(null);
    try {
      await saveSessionBookmark(session.app, session.sessionId, messageId);
      setRevision((value) => value + 1);
      return true;
    } catch (caught) { setError(caught); return false; }
    finally { locked.current = false; setBusy(false); }
  };

  const remove = async (id: string) => {
    if (locked.current || loading) return false;
    locked.current = true;
    setBusy(true);
    setError(null);
    try {
      const result = await deleteSessionBookmark(id);
      if (result.warning) toast({ kind: "warning", title: toastMessage("sessions.bookmarks.reclaimWarning", { detail: result.warning }) });
      setRevision((value) => value + 1);
      return true;
    } catch (caught) { setError(caught); return false; }
    finally { locked.current = false; setBusy(false); }
  };

  const refresh = useCallback(() => { if (!locked.current) setRevision((value) => value + 1); }, []);
  return { items: result?.items ?? null, result, page, setPage, busy, loading, error, save, remove, refresh };
}

export type SessionBookmarks = ReturnType<typeof useSessionBookmarks>;
