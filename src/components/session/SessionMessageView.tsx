import { useEffect, useId, useRef, useState } from "react";
import type { SessionContentPage, SessionMessagePreview } from "../../api/client";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Time } from "../Time";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { copyText, messageRole } from "./session-content";
import { toast, toastMessage } from "../use-toast";
import { SessionContent, SessionText } from "./SessionContent";
import type { ReadingTarget } from "./use-session-detail";

interface Props {
  message: Omit<SessionMessagePreview, "contentRevision">;
  readContent: (offset: number) => Promise<SessionContentPage>;
  readCopyText: (id: string) => Promise<string>;
  target?: ReadingTarget;
  navigate?: boolean;
  expanded: boolean;
  initialOffset?: number;
  onToggleExpanded?: (id: string) => void;
  onBookmark?: () => void;
  bookmarked?: boolean | null;
  bookmarkBusy?: boolean;
}

function MessageHeader({ message, readCopyText, onBookmark, bookmarked, bookmarkBusy }: Pick<Props,
  "message" | "readCopyText" | "onBookmark" | "bookmarked" | "bookmarkBusy">) {
  const { t } = useI18n();
  const [copying, setCopying] = useState(false);
  const [error, setError] = useMessageState();
  const isLong = message.contentLength > [...message.content].length || message.imageCount > 0;
  const copy = async () => {
    setCopying(true); setError(null);
    try {
      await copyText(isLong ? () => readCopyText(message.id) : message.content);
      toast({ kind: "success", title: toastMessage("sessions.message.copied") });
    } catch (caught) { setError(caught); }
    finally { setCopying(false); }
  };
  return <><header>
    <span>{messageRole(message.role)}</span>
    <span className="asb-session-message-time">{message.at ? <Time iso={message.at} /> : null}</span>
    {onBookmark && (bookmarked
      ? <span className="asb-session-message-saved">{t("sessions.bookmarks.saved")}</span>
      : <Button variant="unstyled" className="asb-session-message-bookmark" disabled={bookmarkBusy || bookmarked === null}
        onClick={onBookmark}>{t("sessions.bookmarks.save")}</Button>)}
    <Button variant="unstyled" className="asb-session-message-copy" disabled={copying} onClick={() => void copy()}>
      {t(isLong ? "sessions.message.copyFull" : "sessions.message.copy")}</Button>
  </header>{error && <p className="asb-warn-text" role="alert">{error}</p>}</>;
}

/** Renders one read-only transcript entry and owns its local copy feedback. */
export function SessionMessageView({
  message,
  readContent,
  readCopyText,
  target,
  navigate = false,
  expanded,
  initialOffset = 0,
  onToggleExpanded,
  onBookmark,
  bookmarked,
  bookmarkBusy,
}: Props) {
  const { t } = useI18n();
  const isLong = message.contentLength > [...message.content].length || message.imageCount > 0;
  // Preview and full reader render the same text, so only one may exist at a
  // time: the preview returns exactly when the collapse transition ends and
  // the shared disclosure unmounts the reader (Collapsible onClosed).
  const [previewShown, setPreviewShown] = useState(!expanded);
  const bodyId = useId();
  const article = useRef<HTMLElement>(null);
  useEffect(() => {
    if (expanded) setPreviewShown(false);
  }, [expanded]);
  useEffect(() => {
    if (!navigate || !target || target.match || isLong) return;
    article.current?.focus({ preventScroll: true });
    article.current?.scrollIntoView({ block: "center", inline: "nearest" });
  }, [target, navigate, isLong]);

  return (
    <article
      ref={article}
      data-message-id={message.id}
      className={`asb-session-message is-${message.role.toLowerCase()}${target ? " is-target" : ""}`}
      tabIndex={target ? -1 : undefined}
    >
      <MessageHeader message={message} readCopyText={readCopyText}
        onBookmark={onBookmark} bookmarked={bookmarked} bookmarkBusy={bookmarkBusy} />
      {!isLong ? <SessionText content={message.content} match={target?.match} navigate={navigate} />
        : previewShown ? <SessionText content={message.content} preview /> : null}
      {isLong && <Collapsible open={expanded} id={bodyId} onClosed={() => setPreviewShown(true)}>
        <SessionContent read={readContent} total={message.contentLength} sliceSize={message.contentPageSize}
          target={expanded ? target : undefined} initialOffset={initialOffset} navigate={expanded && navigate} />
      </Collapsible>}
      {isLong && onToggleExpanded && (
        <Button
          variant="unstyled"
          className="asb-session-message-toggle"
          aria-expanded={expanded}
          aria-controls={expanded ? bodyId : undefined}
          onClick={() => onToggleExpanded(message.id)}
        >
          {expanded
            ? t("sessions.message.collapse")
            : message.imageCount > 0 ? t("sessions.message.expandImages", { count: message.imageCount })
            : t("sessions.message.expand", { size: Math.ceil(message.contentLength / 1000) })}
        </Button>
      )}
    </article>
  );
}
