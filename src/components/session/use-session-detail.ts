import { useCallback, useEffect, useRef, useState } from "react";
import { getSessionMessagePage, type SessionMessagePage, type SessionMeta } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import { sessionKey } from "./session-content";
import { useReadQueue } from "../../hooks/use-read-queue";

export function useSessionDetail(active: boolean) {
  const [selected, setSelected] = useState<SessionMeta | null>(null);
  const [page, setPage] = useState<SessionMessagePage | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [contentOffsets, setContentOffsets] = useState<Record<string, number>>({});
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const generation = useRef(0);
  const metadataGeneration = useRef(0);
  const position = useRef({ offset: 0, anchorId: null as string | null });
  const scrollTop = useRef(0);
  const enqueue = useReadQueue();
  const [revision, setRevision] = useState(0);
  const app = selected?.app;
  const sessionId = selected?.sessionId;
  useEffect(() => {
    setPage(null);
    if (!active || !app || !sessionId) { setBusy(false); return; }
    let current = true;
    const version = ++generation.current;
    const metadataVersion = metadataGeneration.current;
    setError(null);
    setBusy(true);
    const run = async () => {
      if (!current) return;
      try {
        const next = await getSessionMessagePage(app, sessionId, position.current.offset, position.current.anchorId);
        if (!current || generation.current !== version) return;
        position.current = { offset: next.offset, anchorId: null };
        if (metadataVersion === metadataGeneration.current) setSelected(next.session);
        setContentOffsets((current) => Object.fromEntries(next.items.map((item) => [item.id, current[item.id] ?? 0])));
        setExpanded((current) => new Set(next.items.filter((item) => current.has(item.id)).map((item) => item.id)));
        setPage(next);
      } finally { if (current) setBusy(false); }
    };
    const cancel = enqueue(run, (caught) => { if (current) setError(caught); });
    return () => { current = false; generation.current += 1; cancel(); };
  }, [active, app, sessionId, revision, setError, enqueue]);
  const select = useCallback((session: SessionMeta, messageId: string | null = null, contentOffset = 0) => {
    generation.current += 1;
    position.current = { offset: 0, anchorId: messageId };
    scrollTop.current = 0;
    setContentOffsets(messageId ? { [messageId]: contentOffset } : {});
    setExpanded(new Set(messageId ? [messageId] : []));
    setSelected(session); setTarget(messageId); setPage(null); setRevision((value) => value + 1);
  }, []);
  const clear = () => {
    generation.current += 1;
    setSelected(null); setPage(null); setTarget(null); setBusy(false); setError(null);
    setContentOffsets({}); setExpanded(new Set());
  };
  const updateMetadata = (sessions: SessionMeta[]) => {
    metadataGeneration.current += 1;
    setSelected((current) => current
      ? sessions.find((session) => sessionKey(session) === sessionKey(current)) ?? current : null);
  };
  const changePage = (next: number) => {
    if (!page || busy) return;
    position.current = { offset: (next - 1) * page.pageSize, anchorId: null };
    scrollTop.current = 0; setTarget(null); setRevision((value) => value + 1);
    setContentOffsets({}); setExpanded(new Set());
  };
  const markBookmarked = (id: string) => setPage((current) => current ? { ...current,
    items: current.items.map((item) => item.id === id ? { ...item, bookmarked: true } : item) } : null);
  const refresh = () => setRevision((value) => value + 1);
  const setContentOffset = (id: string, offset: number) => setContentOffsets((current) => ({ ...current, [id]: offset }));
  return { selected, page, target, setTarget, scrollTop, contentOffsets, setContentOffset, expanded, setExpanded, busy, error,
    select, clear, updateMetadata, changePage, markBookmarked, refresh };
}

export type SessionDetailState = ReturnType<typeof useSessionDetail>;
