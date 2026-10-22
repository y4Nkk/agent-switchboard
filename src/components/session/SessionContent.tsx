import { useEffect, useState } from "react";
import type { SessionContentPage } from "../../api/client";
import { MarkdownContent } from "../Markdown";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { Pagination } from "../Pagination";
import { toast, toastMessage } from "../use-toast";
import { copyText } from "./session-content";
import { useReadQueue } from "../../hooks/use-read-queue";

function SessionImage({ src, index }: { src: string; index: number }) {
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  return failed ? <p className="asb-warn-text" role="alert">{t("sessions.message.imageUnavailable", { index: index + 1 })}</p>
    : <img src={src} alt={t("sessions.message.imageAlt", { index: index + 1 })} loading="lazy" onError={() => setFailed(true)} />;
}

export function SessionText({ content, partial = false, preview = false }: {
  content: string; partial?: boolean; preview?: boolean;
}) {
  const { t } = useI18n();
  const copy = async () => {
    try { await copyText(content); toast({ kind: "success", title: toastMessage("sessions.message.copied") }); }
    catch { toast({ kind: "error", title: toastMessage("sessions.message.copyFailed") }); }
  };
  return <>
    <div className="asb-session-message-body asb-markdown">
      {partial || preview ? <pre className="asb-session-plain-content">{content}{preview ? "…" : ""}</pre>
        : <MarkdownContent text={content} />}
    </div>
    {(partial || preview) && <Button variant="unstyled" className="asb-session-message-copy" onClick={() => void copy()}>
      {t(preview ? "sessions.message.copyPreview" : partial ? "sessions.message.copyPage" : "sessions.message.copy")}
    </Button>}
  </>;
}

export function SessionContent({ read, offset, onOffsetChange }: {
  read: (offset: number) => Promise<SessionContentPage>; offset: number; onOffsetChange: (offset: number) => void;
}) {
  const { t } = useI18n();
  const [part, setPart] = useState<SessionContentPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useMessageState();
  const enqueue = useReadQueue();
  useEffect(() => {
    let current = true;
    setPart(null); setLoading(true); setError(null);
    const run = async () => {
      if (!current) return;
      try { const next = await read(offset); if (current) setPart(next); }
      finally { if (current) setLoading(false); }
    };
    const cancel = enqueue(run, (caught) => { if (current) setError(caught); });
    return () => { current = false; cancel(); };
  }, [read, offset, setError, enqueue]);
  return <div aria-busy={loading}>
    {loading && <p role="status">{t("sessions.transcript.loading.aria")}</p>}
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    {part && <>
      {part.issues.map((issue, index) => <p key={index} className="asb-warn-text" role="alert">{issue}</p>)}
      {part.images.length > 0 && <div className="asb-session-images">{part.images.map((src, index) =>
        <SessionImage key={index} src={src} index={index} />)}</div>}
      {part.total > part.pageSize && <p className="asb-scope-note">{t("sessions.message.pagedText")}</p>}
      <SessionText content={part.content} partial={part.total > part.pageSize} />
      <Pagination total={part.total} page={Math.floor(part.offset / part.pageSize) + 1} pageSize={part.pageSize}
        label={t("sessions.message.pagination")} onPageChange={(page) => onOffsetChange((page - 1) * part.pageSize)} />
    </>}
  </div>;
}
