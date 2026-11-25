import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { getSessionFirstMessages, getSessionMessagePage, type SessionFirstMessage,
  type SessionMessagePage, type SessionMeta, type TranscriptMatch } from "../../api/client";
import { uiMessage } from "../../i18n/errors";
import { useMessageState } from "../../i18n/use-message-state";
import { sessionKey } from "./session-content";
import { useReadQueue } from "../../hooks/use-read-queue";
import type { ReadingPosition } from "./ReadingWindow";

type PageAnchor = { messageId: string; contentRevision?: string };
type PreviewAnchor = { id: string; inset: number };
export type ReadingTarget = { messageId: string; contentOffset: number; batchIndex?: number; match?: TranscriptMatch };
const FIRST_PAGE = { offset: 0, anchor: null as PageAnchor | null };

async function locatePreviewAnchor(page: SessionMessagePage, visible: RefObject<PreviewAnchor | null>) {
  const anchor = visible.current;
  if (!anchor || page.items.some((item) => item.id === anchor.id)) return page;
  const { app, sessionId } = page.session;
  const located = await getSessionMessagePage(app, sessionId, page.offset, anchor.id, page.snapshot).catch(() => null);
  return located && visible.current?.id === anchor.id ? located : page;
}

function useTranscriptPage(active: boolean, selected: SessionMeta | null, metadataGeneration: RefObject<number>,
  readingPosition: RefObject<ReadingPosition>,
  applyPage: (page: SessionMessagePage, metadataCurrent: boolean) => void, clearTarget: () => void) {
  const [page, setPage] = useState<SessionMessagePage | null>(null);
  const [preview, setPreview] = useState<SessionFirstMessage[]>([]);
  const previewAnchor = useRef<PreviewAnchor | null>(null);
  const [restoredPreview, setRestoredPreview] = useState<PreviewAnchor | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const generation = useRef(0);
  const position = useRef(FIRST_PAGE);
  const enqueue = useReadQueue();
  const [revision, setRevision] = useState(0);
  const app = selected?.app;
  const sessionId = selected?.sessionId;
  useEffect(() => {
    if (!active || !app || !sessionId) {
      if (!position.current.anchor) clearTarget();
      setPage(null); setPreview([]); previewAnchor.current = null; setRestoredPreview(null); setBusy(false); return;
    }
    const version = ++generation.current;
    const metadataVersion = metadataGeneration.current;
    const { offset, anchor } = position.current;
    const isCurrent = () => generation.current === version;
    setError(null);
    setBusy(true);
    const first = offset === 0 && !anchor && readingPosition.current.index === 0
      ? getSessionFirstMessages(app, sessionId).catch(() => [])
        .then((messages) => { if (isCurrent()) setPreview(messages); })
      : null;
    const run = async () => {
      try {
        if (first && isCurrent()) await first;
        if (!isCurrent()) return;
        let next = await getSessionMessagePage(app, sessionId, offset, anchor?.messageId ?? null, null);
        if (!isCurrent()) return;
        if (anchor?.contentRevision && !next.items.some((item) =>
          item.id === anchor.messageId && item.contentRevision === anchor.contentRevision)) {
          position.current = { offset, anchor: null };
          clearTarget(); setError(uiMessage("sessions.find.changed"));
          return;
        }
        if (!anchor && previewAnchor.current) {
          next = await locatePreviewAnchor(next, previewAnchor);
          if (!isCurrent()) return;
        }
        const visible = previewAnchor.current;
        const restored = visible && next.items.some((item) => item.id === visible.id) ? visible : null;
        if (restored) readingPosition.current = { index: next.offset / next.pageSize, inset: 0 };
        setRestoredPreview(restored);
        previewAnchor.current = null;
        setPreview([]);
        position.current = { offset: next.offset, anchor: null };
        applyPage(next, metadataVersion === metadataGeneration.current);
        setPage(next);
      } finally { if (isCurrent()) setBusy(false); }
    };
    const cancel = enqueue(run, (caught) => {
      if (!isCurrent()) return;
      if (anchor) {
        position.current = { offset, anchor: null };
        clearTarget();
      }
      setError(caught);
    });
    return () => { generation.current += 1; cancel(); };
  }, [active, app, sessionId, revision, metadataGeneration, readingPosition, applyPage, clearTarget, setError, enqueue]);
  const requestPage = useCallback((offset: number, anchor: PageAnchor | null) => {
    generation.current += 1;
    position.current = { offset, anchor };
    setBusy(true); setRevision((value) => value + 1);
  }, []);
  const resetRead = useCallback(() => {
    generation.current += 1;
    position.current = FIRST_PAGE;
    previewAnchor.current = null; setRestoredPreview(null);
    setPage(null); setPreview([]); setBusy(false); setError(null);
  }, [setError]);
  return { page, preview, previewAnchor, restoredPreview, busy, error, setError, position, requestPage, resetRead };
}

export function useSessionDetail(active: boolean) {
  const [selected, setSelected] = useState<SessionMeta | null>(null);
  const [target, setTarget] = useState<ReadingTarget | null>(null);
  const [savedMessages, setSavedMessages] = useState<ReadonlySet<string>>(new Set());
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [selectionRevision, setSelectionRevision] = useState(0);
  const [refreshRevision, setRefreshRevision] = useState(0);
  const readingPosition = useRef<ReadingPosition>({ index: 0, inset: 0 });
  const metadataGeneration = useRef(0);
  const clearTarget = useCallback(() => setTarget(null), []);
  const applyPage = useCallback((next: SessionMessagePage, metadataCurrent: boolean) => {
    if (metadataCurrent) setSelected(next.session);
  }, []);
  const { page, preview, previewAnchor, restoredPreview, busy, error, setError, position, requestPage, resetRead } =
    useTranscriptPage(active, selected, metadataGeneration, readingPosition, applyPage, clearTarget);

  const select = useCallback((session: SessionMeta, messageId: string | null = null, contentOffset = 0) => {
    resetRead(); requestPage(0, messageId ? { messageId } : null);
    readingPosition.current = { index: 0, inset: 0 };
    setSavedMessages(new Set());
    setExpanded(new Set(messageId ? [messageId] : []));
    setSelected(session); setTarget(messageId ? { messageId, contentOffset } : null);
    setSelectionRevision((value) => value + 1);
  }, [requestPage, resetRead]);
  const clear = () => {
    resetRead(); setSelected(null); setTarget(null);
    setSavedMessages(new Set()); setExpanded(new Set());
  };
  const updateMetadata = (sessions: SessionMeta[]) => {
    metadataGeneration.current += 1;
    setSelected((current) => current
      ? sessions.find((session) => sessionKey(session) === sessionKey(current)) ?? current : null);
  };
  const revealMatch = useCallback((match: TranscriptMatch) => {
    const { messageId, contentOffset } = match;
    setError(null);
    setTarget({ messageId, contentOffset, match: { ...match } });
    const offset = page?.offset ?? position.current.offset;
    const onPage = page?.items.some((item) => item.id === messageId && item.contentRevision === match.contentRevision);
    setExpanded((current) => new Set([...current, messageId]));
    if (!onPage) requestPage(offset, match);
    else if (position.current.anchor || position.current.offset !== offset) requestPage(offset, null);
  }, [page, position, requestPage, setError]);
  const clearMatch = useCallback(() => {
    setTarget((current) => current?.match ? null : current);
    if (position.current.anchor?.contentRevision) requestPage(position.current.offset, null);
  }, [position, requestPage]);
  const revealOutline = (messageId: string, batchIndex: number) => {
    if (position.current.anchor) requestPage(page?.offset ?? position.current.offset, null);
    setError(null);
    setTarget({ messageId, contentOffset: 0, batchIndex });
    setExpanded((current) => new Set([...current, messageId]));
  };
  const markBookmarked = (id: string) => setSavedMessages((current) => new Set([...current, id]));
  const refresh = () => {
    setTarget(null);
    const offset = page ? readingPosition.current.index * page.pageSize : position.current.offset;
    resetRead();
    requestPage(offset, null);
    setRefreshRevision((value) => value + 1);
  };
  return { active, selected, page, preview, previewAnchor, restoredPreview,
    target, setTarget, readingPosition, expanded, setExpanded, busy, error, setError, savedMessages,
    select, clear, updateMetadata, markBookmarked, refresh, revealMatch, revealOutline, clearMatch, selectionRevision, refreshRevision };
}

export type SessionDetailState = ReturnType<typeof useSessionDetail>;
