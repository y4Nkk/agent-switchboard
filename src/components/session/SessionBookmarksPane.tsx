import { useCallback, useEffect, useRef, useState } from "react";
import { ActionMenu } from "../ActionMenu";
import { getSessionBookmarkContent, getSessionCopyText, getSessionMetadata, type SessionBookmarkSummary, type SessionMeta } from "../../api/client";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { Pagination } from "../Pagination";
import { ClientLogo } from "../ClientLogo";
import { ConfirmSheet } from "../ConfirmSheet";
import { Time } from "../Time";
import { ScrollArea } from "../ScrollArea";
import { SessionMessageView } from "./SessionMessageView";
import type { SessionBookmarks } from "./use-session-bookmarks";
import { useReadingScroll } from "./use-reading-scroll";

function BookmarkDetail({ item, offset, scrollTop, onOffsetChange, busy, onRemove, onOpen }: {
  item: SessionBookmarkSummary; busy: boolean; onRemove: () => void;
  offset: number; onOffsetChange: (id: string, offset: number) => void;
  scrollTop: { current: number };
  onOpen: (session: SessionMeta, messageId: string, contentOffset: number) => void;
}) {
  const { t } = useI18n();
  const [opening, setOpening] = useState(false);
  const [error, setError] = useMessageState();
  const mounted = useRef(true);
  const { container, onScroll } = useReadingScroll(scrollTop);
  const readContent = useCallback((_id: string, offset: number) => getSessionBookmarkContent(item.id, offset), [item.id]);
  const readCopyText = useCallback((_id: string) => getSessionCopyText({ kind: "bookmark", id: item.id }), [item.id]);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const open = async () => {
    setOpening(true); setError(null);
    try {
      const source = await getSessionMetadata(item.app, item.sessionId);
      if (!mounted.current) return;
      onOpen(source, item.messageId, offset);
    } catch (caught) { if (mounted.current) setError(caught); }
    finally { if (mounted.current) setOpening(false); }
  };
  return <ScrollArea as="section" className="asb-session-detail" aria-label={t("sessions.bookmarks.detail")}>
    <header className="asb-session-detail-head">
      <div className="asb-session-detail-title"><h3 className="asb-section-title" title={item.sessionTitle}>{item.sessionTitle}</h3>
        <span className="asb-scope-note">{t("sessions.bookmarks.savedAt")} <Time iso={item.savedAt} /></span>
      </div>
      <div className="asb-session-actions">
        <Button variant="primary" disabled={opening} onClick={() => void open()}>{t("sessions.bookmarks.openSource")}</Button>
        <ActionMenu label={t("sessions.actions.more")} ariaLabel={t("sessions.actions.moreAria")}
          disabled={busy} items={[{ id: "remove", label: t("sessions.bookmarks.remove"), danger: true, onAction: onRemove }]} />
      </div>
    </header>
    <p className="asb-scope-note">{t("sessions.bookmarks.snapshot")}</p>
    {item.projectDir && <p className="asb-session-bookmark-path">{item.projectDir}</p>}
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    <ScrollArea className="asb-session-transcript" viewportRef={container} onScroll={onScroll}
      aria-label={t("sessions.bookmarks.detail")}>
      <SessionMessageView message={{ id: item.messageId, role: item.role, content: item.excerpt,
        contentLength: item.contentLength, bookmarked: true, at: item.at, imageCount: item.imageCount }}
        readContent={readContent} readCopyText={readCopyText} targeted={false} expanded contentOffset={offset}
        onContentOffsetChange={(_id, next) => onOffsetChange(item.id, next)} />
    </ScrollArea>
  </ScrollArea>;
}

export function SessionBookmarksPane({ bookmarks, onOpen }: {
  bookmarks: SessionBookmarks; onOpen: (session: SessionMeta, messageId: string, contentOffset: number) => void;
}) {
  const { t } = useI18n();
  const [reading, setReading] = useState<{ id: string | null; offset: number }>({ id: null, offset: 0 });
  const scrollTop = useRef(0);
  const [pending, setPending] = useState<SessionBookmarkSummary | null>(null);
  const items = bookmarks.items ?? [];
  const selected = items.find((item) => item.id === reading.id);
  return <>
    <div className="asb-session-layout">
      <section className="asb-session-list" aria-label={t("sessions.bookmarks.title")} aria-busy={bookmarks.loading}>
        <div className="asb-session-list-heading"><span>{t("sessions.bookmarks.title")}</span>
          <span className="asb-session-count">{bookmarks.result?.total ?? 0}</span></div>
        {bookmarks.loading && <p role="status">{t("sessions.bookmarks.loading")}</p>}
        {!bookmarks.loading && bookmarks.items !== null && !items.length && <p>{t("sessions.bookmarks.empty")}</p>}
        <ScrollArea className="asb-session-items" aria-label={t("sessions.bookmarks.title")}>{items.map((item) =>
          <Button variant="unstyled" key={item.id} aria-pressed={reading.id === item.id}
            className={`asb-session-item${reading.id === item.id ? " is-active" : ""}`}
            onClick={() => { scrollTop.current = 0; setReading({ id: item.id, offset: item.contentOffset }); }}>
            <span className="asb-session-item-title"><ClientLogo app={item.app} className="asb-session-logo" />
              <span title={item.sessionTitle}>{item.sessionTitle}</span></span>
            <span className="asb-session-search-excerpt">{item.excerpt.slice(0, 220)}</span>
            <span className="asb-session-item-time"><Time iso={item.savedAt} /></span>
          </Button>)}</ScrollArea>
        {bookmarks.result && <Pagination total={bookmarks.result.total} page={bookmarks.page}
          pageSize={bookmarks.result.pageSize} label={t("sessions.bookmarks.pagination")} onPageChange={bookmarks.setPage} />}
      </section>
      {selected ? <BookmarkDetail key={selected.id} item={selected} busy={bookmarks.busy || bookmarks.loading} onOpen={onOpen}
        offset={reading.offset} scrollTop={scrollTop} onOffsetChange={(id, offset) => setReading({ id, offset })}
        onRemove={() => setPending(selected)} /> : <ScrollArea as="section" className="asb-session-detail asb-session-empty-detail"
          aria-label={t("sessions.bookmarks.detail")}><p>{t("sessions.bookmarks.select")}</p></ScrollArea>}
    </div>
    {pending && <ConfirmSheet title={t("sessions.bookmarks.remove")} confirmLabel={t("sessions.bookmarks.remove")}
      busy={bookmarks.busy}
      confirmDisabled={bookmarks.busy || bookmarks.loading}
      destructive onCancel={() => setPending(null)} onConfirm={() => {
        void bookmarks.remove(pending.id).then((removed) => { if (removed) setPending(null); });
      }}>
      <p>{t("sessions.bookmarks.removeDetail", { title: pending.sessionTitle })}</p>
    </ConfirmSheet>}
  </>;
}
