import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { SessionContentPage, TranscriptMatch } from "../../api/client";
import { MarkdownContent } from "../Markdown";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { toast, toastMessage } from "../use-toast";
import { copyText } from "./session-content";
import { ReadingPlaceholder, ReadingWindow } from "./ReadingWindow";
import { useReadingBlock } from "./use-reading-block";
import type { ReadingTarget } from "./use-session-detail";

function SessionImage({ src, index }: { src: string; index: number }) {
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  return failed ? <p className="asb-warn-text" role="alert">{t("sessions.message.imageUnavailable", { index: index + 1 })}</p>
    : <img src={src} alt={t("sessions.message.imageAlt", { index: index + 1 })} loading="lazy" onError={() => setFailed(true)} />;
}

function MatchedText({ content, match, offset, navigate }: {
  content: string; match: TranscriptMatch; offset: number; navigate: boolean;
}) {
  const marker = useRef<HTMLElement>(null);
  const chars = [...content];
  const start = Math.max(0, match.start - offset);
  const end = Math.min(chars.length, match.end - offset);
  useEffect(() => {
    const mark = marker.current;
    const viewport = mark?.closest<HTMLElement>(".asb-session-transcript > .asb-scroll-viewport");
    if (!navigate || !mark || !viewport) return;
    const center = () => {
      const line = mark.getClientRects()[0];
      if (!line) return;
      viewport.scrollTop += line.top - viewport.getBoundingClientRect().top - viewport.clientHeight / 2;
    };
    const frame = window.requestAnimationFrame(center);
    return () => window.cancelAnimationFrame(frame);
  }, [match, offset, content, navigate]);
  return <pre className="asb-session-plain-content">{chars.slice(0, start).join("")}
    <mark ref={marker}>{chars.slice(start, end).join("")}</mark>{chars.slice(end).join("")}</pre>;
}

export function SessionText({ content, partial = false, preview = false, match, offset = 0, navigate = false }: {
  content: string; partial?: boolean; preview?: boolean; match?: TranscriptMatch; offset?: number; navigate?: boolean;
}) {
  const { t } = useI18n();
  const copy = async () => {
    try { await copyText(content); toast({ kind: "success", title: toastMessage("sessions.message.copied") }); }
    catch { toast({ kind: "error", title: toastMessage("sessions.message.copyFailed") }); }
  };
  const highlighted = match && match.end > offset && content.length > 0
    && (offset >= match.start || match.start - offset < [...content].length) ? match : undefined;
  return <>
    <div className="asb-session-message-body asb-markdown">
      {highlighted ? <MatchedText content={content} match={highlighted} offset={offset} navigate={navigate} />
        : partial || preview ? <pre className="asb-session-plain-content">{content}{preview ? "…" : ""}</pre>
        : <MarkdownContent text={content} />}
    </div>
    {preview && <Button variant="unstyled" className="asb-session-message-copy" onClick={() => void copy()}>
      {t("sessions.message.copyPreview")}
    </Button>}
  </>;
}

function ContentBlock({ read, offset, match, partial }: {
  read: (offset: number) => Promise<SessionContentPage>; offset: number; match?: TranscriptMatch; partial: boolean;
}) {
  const load = useCallback(() => read(offset), [read, offset]);
  const { value, busy, error, retry } = useReadingBlock(load);
  return <div aria-busy={busy} className={partial ? "asb-continuous-text" : undefined}>
    {!value ? <ReadingPlaceholder error={error} retry={retry} /> : <>
      {value.issues.map((issue, index) => <p key={index} className="asb-warn-text" role="alert">{issue}</p>)}
      {value.images.length > 0 && <div className="asb-session-images">{value.images.map((src, index) =>
        <SessionImage key={index} src={src} index={index} />)}</div>}
      <SessionText content={value.content} partial={partial} match={match} offset={value.offset} />
    </>}
  </div>;
}

export function SessionContent({ read, total, sliceSize, target, initialOffset, navigate }: {
  read: (offset: number) => Promise<SessionContentPage>; total: number; sliceSize: number;
  target?: ReadingTarget; initialOffset: number; navigate: boolean;
}) {
  const count = Math.max(1, Math.ceil(total / sliceSize));
  const offset = navigate ? target?.contentOffset ?? 0 : initialOffset;
  const jump = useMemo(() => navigate || offset > 0 ? {
    index: Math.floor(offset / sliceSize), afterContent: target?.match ? "match" as const : "start" as const,
  } : null,
    [offset, sliceSize, target, navigate]);
  return <ReadingWindow count={count} estimate={count > 1 ? 3000 : 240} jump={jump}>
    {(index) => <ContentBlock read={read} offset={index * sliceSize} match={target?.match} partial={count > 1} />}
  </ReadingWindow>;
}
