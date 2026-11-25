import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { getSessionCopyText, getSessionMessageContent, getSessionMessagePage,
  type SessionMessagePage, type SessionMessagePreview, type SessionMeta } from "../../api/client";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { ScrollArea } from "../ScrollArea";
import { Time } from "../Time";
import { ReadingPlaceholder, ReadingWindow } from "./ReadingWindow";
import { SessionMessageView } from "./SessionMessageView";
import { SessionText } from "./SessionContent";
import { codexOutlinePreview, messageRole, previewLine } from "./session-content";
import type { SessionBookmarks } from "./use-session-bookmarks";
import type { ReadingTarget, SessionDetailState } from "./use-session-detail";
import { useReadingBlock } from "./use-reading-block";

interface Props {
  session: SessionMeta;
  detail: SessionDetailState;
  bookmarks: SessionBookmarks;
}

interface OutlineEntry {
  id: string;
  preview: string;
  index: number;
}

function outline(messages: SessionMessagePreview[], app: SessionMeta["app"], index: number): OutlineEntry[] {
  return messages.filter((message) => message.role === "user").flatMap((message) => {
    const content = app === "codex" ? codexOutlinePreview(message.content) : message.content;
    return content === null ? [] : [{ id: message.id, preview: previewLine(content), index }];
  });
}

function TranscriptEntry({ session: { app, sessionId }, detail, message, bookmarkBusy, onBookmark, seen }: {
  session: SessionMeta; detail: SessionDetailState; message: SessionMessagePreview; bookmarkBusy: boolean;
  onBookmark: (id: string) => void; seen: WeakSet<ReadingTarget>;
}) {
  const { id, contentRevision } = message;
  const candidate = detail.target?.messageId === id ? detail.target : undefined;
  const target = candidate?.match && candidate.match.contentRevision !== contentRevision ? undefined : candidate;
  const arrived = useRef<ReadingTarget | null>(null);
  const navigate = !!target && (arrived.current === target || !seen.has(target));
  useEffect(() => {
    if (!target || !navigate) return;
    arrived.current = target;
    seen.add(target);
  }, [target, navigate, seen]);
  const readContent = useCallback((offset: number) =>
    getSessionMessageContent(app, sessionId, id, offset, contentRevision), [app, sessionId, id, contentRevision]);
  const readCopyText = useCallback((messageId: string) => getSessionCopyText({ kind: "session", app,
    sessionId, messageId }), [app, sessionId]);
  const toggle = () => {
    if (target) detail.setTarget(null);
    detail.setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };
  return <SessionMessageView message={message} target={target} navigate={navigate}
    expanded={detail.expanded.has(id)} readContent={readContent} readCopyText={readCopyText}
    onToggleExpanded={toggle} bookmarked={detail.savedMessages.has(id) || message.bookmarked}
    bookmarkBusy={bookmarkBusy} onBookmark={["user", "assistant"].includes(message.role) ? () => onBookmark(id) : undefined} />;
}

function TranscriptBatch({ session, detail, seed, index, reportOutline, bookmarkBusy, onBookmark, seen }: {
  session: SessionMeta; detail: SessionDetailState; seed: SessionMessagePage; index: number;
  reportOutline: (index: number, items: OutlineEntry[] | null) => void;
  bookmarkBusy: boolean; onBookmark: (id: string) => void; seen: WeakSet<ReadingTarget>;
}) {
  const { t } = useI18n();
  const { app, sessionId } = session;
  const { total, revision } = seed.snapshot;
  const offset = index * seed.pageSize;
  const load = useCallback(() => getSessionMessagePage(app, sessionId, offset, null, { total, revision }),
    [app, sessionId, offset, total, revision]);
  const { value, busy, error } = useReadingBlock(load, offset === seed.offset ? seed : undefined);
  useEffect(() => {
    if (!value) return;
    reportOutline(index, outline(value.items, app, index));
    return () => reportOutline(index, null);
  }, [app, index, value, reportOutline]);
  if (!value) return <div aria-busy={busy}><ReadingPlaceholder error={error} retry={detail.refresh} /></div>;
  return <div className="asb-session-message-batch">
    {value.bookmarkIssue && <p className="asb-warn-text" role="alert">{localizedMessageText(value.bookmarkIssue, t)}</p>}
    {value.issues.map((issue, index) => <p key={index} className="asb-warn-text" role="alert">{issue}</p>)}
    {value.items.map((message) => <TranscriptEntry key={message.id} session={session} detail={detail}
      message={message} bookmarkBusy={bookmarkBusy} onBookmark={onBookmark} seen={seen} />)}
  </div>;
}

function SessionOutline({ items, onSelect }: { items: OutlineEntry[]; onSelect: (item: OutlineEntry) => void }) {
  const { t } = useI18n();
  if (items.length === 0) return null;
  return <nav className="asb-session-toc" aria-label={t("sessions.toc.aria")}>
    <div className="asb-session-toc-heading">{t("sessions.toc.heading")}</div>
    <ScrollArea className="asb-session-toc-items" aria-label={t("sessions.toc.aria")}>{items.map((item, index) =>
      <Button variant="unstyled" key={item.id} onClick={() => onSelect(item)}>
        <span className="asb-session-toc-index">{index + 1}</span>
        <span className="asb-session-toc-preview" title={item.preview}>{item.preview}</span>
      </Button>)}</ScrollArea>
  </nav>;
}

export function SessionTranscriptPreview({ detail }: { detail: SessionDetailState }) {
  const { t } = useI18n();
  return <div className="asb-session-body">
    <div className="asb-session-conversation">
      <div className="asb-session-list-heading">
        <span>{t("sessions.transcript.heading")}</span>
        {detail.busy && <span role="status">{t("sessions.transcript.loadingRest")}</span>}
      </div>
      <ScrollArea className="asb-session-transcript" aria-label={t("sessions.transcript.aria")}
        onScroll={({ currentTarget: viewport }) => {
          if (viewport.scrollTop < 1) { detail.previewAnchor.current = null; return; }
          const top = viewport.getBoundingClientRect().top;
          const first = Array.from(viewport.querySelectorAll<HTMLElement>("[data-message-id]"))
            .find((message) => message.getBoundingClientRect().bottom > top);
          if (first?.dataset.messageId) detail.previewAnchor.current = { id: first.dataset.messageId,
            inset: top - first.getBoundingClientRect().top };
        }}>
        <div className="asb-session-message-batch">
          {detail.preview.map((message) => <article key={message.id} data-message-id={message.id}
            className={`asb-session-message is-${message.role.toLowerCase()}`}>
            <header><span>{messageRole(message.role)}</span><span className="asb-session-message-time">
              {message.at && <Time iso={message.at} />}
            </span></header>
            <SessionText content={message.truncated ? `${message.content}…` : message.content} partial />
          </article>)}
        </div>
      </ScrollArea>
    </div>
  </div>;
}

export function SessionTranscript({ session, detail, bookmarks }: Props) {
  const { t } = useI18n();
  const seed = detail.page!;
  const total = seed.snapshot.total;
  const target = detail.target;
  const batchIndex = target?.batchIndex ?? (target && seed.items.some((item) => item.id === target.messageId)
    ? seed.offset / seed.pageSize : undefined);
  const restored = detail.restoredPreview;
  const jump = useMemo(() => batchIndex === undefined ? null : { index: batchIndex }, [target, batchIndex]);
  const viewport = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!restored) return;
    const frame = window.requestAnimationFrame(() => {
      const view = viewport.current;
      if (!view) return;
      const message = Array.from(view.querySelectorAll<HTMLElement>("[data-message-id]"))
        .find((element) => element.dataset.messageId === restored.id);
      if (message) view.scrollTop += message.getBoundingClientRect().top - view.getBoundingClientRect().top
        + Math.min(restored.inset, message.offsetHeight - 1);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [restored]);
  const [nearby, setNearby] = useState<ReadonlyMap<number, OutlineEntry[]>>(() => new Map());
  const seen = useRef(new WeakSet<ReadingTarget>());
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const reportOutline = useCallback((index: number, items: OutlineEntry[] | null) => setNearby((previous) => {
    if (!items && !previous.has(index)) return previous;
    const next = new Map(previous);
    if (items) next.set(index, items); else next.delete(index);
    return next;
  }), []);
  const items = [...nearby].sort(([left], [right]) => left - right).flatMap(([, entries]) => entries);
  const bookmark = (id: string) => {
    void bookmarks.save(session, id).then((saved) => { if (saved && mounted.current) detail.markBookmarked(id); });
  };
  return <div className="asb-session-body">
    <div className="asb-session-conversation">
      <div className="asb-session-list-heading">
        <span>{t("sessions.transcript.heading")}</span><span className="asb-session-count">{total}</span>
      </div>
      <ScrollArea className="asb-session-transcript" viewportRef={viewport} aria-label={t("sessions.transcript.aria")}>
        {total === 0 ? <>
          <p>{t("sessions.transcript.empty")}</p>
          {seed.issues.map((issue, index) => <p key={index} className="asb-warn-text" role="alert">{issue}</p>)}
        </> : <ReadingWindow count={Math.ceil(total / seed.pageSize)} estimate={2000}
          position={detail.readingPosition} jump={jump}>
          {(index) => <TranscriptBatch session={session} detail={detail} seed={seed} index={index}
            reportOutline={reportOutline} bookmarkBusy={bookmarks.busy} onBookmark={bookmark} seen={seen.current} />}
        </ReadingWindow>}
      </ScrollArea>
    </div>
    <SessionOutline items={items} onSelect={(item) => detail.revealOutline(item.id, item.index)} />
  </div>;
}
