import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { searchTranscript, type SessionMeta, type SessionSearchProgress,
  type TranscriptMatch, type TranscriptSearchPage } from "../../api/client";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { Button } from "../Button";
import { Input } from "../Input";
import { Tooltip } from "../Tooltip";

const START = { index: 0, offset: 0, revision: null as string | null };

function useTranscriptFind({ app, sessionId }: SessionMeta, refreshRevision: number) {
  const [draft, setDraft] = useState("");
  const [composing, setComposing] = useState(false);
  const [cursor, setCursor] = useState({ ...START, refreshRevision });
  const request = useMemo(() => cursor.refreshRevision === refreshRevision ? cursor : { ...START, refreshRevision },
    [cursor, refreshRevision]);
  const [snapshot, setSnapshot] = useState<{ query: string; request: typeof request;
    page: TranscriptSearchPage; index: number } | null>(null);
  const [progress, setProgress] = useState<SessionSearchProgress | null>(null);
  const [error, setError] = useMessageState();
  const query = draft.trim();
  const result = !composing && snapshot?.query === query && snapshot.request === request ? snapshot : null;

  useEffect(() => {
    if (composing || !query) return;
    setProgress(null); setError(null);
    const controller = new AbortController();
    const { signal } = controller;
    const run = async () => {
      try {
        const page = await searchTranscript({ app, sessionId, query, offset: request.offset, revision: request.revision },
          signal, (next) => { if (!signal.aborted) setProgress(next); });
        if (!signal.aborted) setSnapshot({ query, request, page, index: request.index });
      } catch (caught) {
        if (!signal.aborted) setError(caught);
      }
    };
    const timer = window.setTimeout(() => void run(), request.revision === null ? 300 : 0);
    return () => { window.clearTimeout(timer); controller.abort(); };
  }, [app, sessionId, query, composing, request, setError]);

  const reset = () => {
    setSnapshot(null); setProgress(null); setError(null); setCursor({ ...START, refreshRevision });
  };
  const change = (value: string) => {
    setDraft(value);
    if (value.trim() !== query) reset();
  };
  const move = (direction: number) => {
    if (!result || result.page.total === 0) return;
    const { page, index } = result;
    const next = (index + direction + page.total) % page.total;
    if (page.hits[next - page.offset]) {
      setSnapshot({ ...result, index: next });
    } else {
      setProgress(null);
      setCursor({ index: next, offset: Math.floor(next / page.pageSize) * page.pageSize, revision: page.revision,
        refreshRevision });
    }
  };

  return { draft, query, composing, setComposing, result, progress, error, change, move, reset };
}

export function SessionFind({ id, session, input, onMatch, onClear, onClose, refreshRevision, navigationFailed, onRetry }: {
  id: string; session: SessionMeta; input: RefObject<HTMLInputElement | null>;
  refreshRevision: number; navigationFailed: boolean;
  onMatch: (match: TranscriptMatch) => void; onClear: () => void; onClose: () => void;
  onRetry: () => void;
}) {
  const { t } = useI18n();
  const find = useTranscriptFind(session, refreshRevision);
  const { result, query, composing, progress, error } = find;
  const applied = useRef<typeof result>(null);
  useEffect(() => {
    if (!result || result === applied.current) return;
    applied.current = result;
    const hit = result.page.hits[result.index - result.page.offset];
    if (hit) onMatch(hit);
  }, [onMatch, result]);
  const status = composing || !query || error ? "" : result
    ? result.page.total > 0 ? t("sessions.find.count", { current: result.index + 1, total: result.page.total })
      : t("sessions.find.empty")
    : progress?.messagesTotal != null && progress.messagesCompleted < progress.messagesTotal
      ? t("sessions.find.progress", { completed: progress.messagesCompleted, total: progress.messagesTotal })
      : t("sessions.find.working");
  const moveDisabled = !result?.page.total;
  const previousLabel = t("sessions.find.previous");
  const nextLabel = t("sessions.find.next");
  const closeLabel = t("sessions.find.close");
  return <div id={id} role="search" aria-label={t("sessions.find.label")} className="asb-session-find"
    onKeyDown={(event) => {
      if (event.key === "Escape" && !event.nativeEvent.isComposing && !composing) {
        event.preventDefault(); event.stopPropagation(); onClose();
      }
    }}>
    <div className="asb-session-find-controls">
      <div className="asb-session-find-input"><Input ref={input} type="search" value={find.draft}
        aria-label={t("sessions.find.label")} placeholder={t("sessions.find.placeholder")}
        onChange={(event) => {
          if (event.target.value.trim() !== query) onClear();
          find.change(event.target.value);
        }}
        onCompositionStart={() => { onClear(); find.setComposing(true); }}
        onCompositionEnd={(event) => { find.change(event.currentTarget.value); find.setComposing(false); }}
        onBlur={() => find.setComposing(false)}
        onKeyDown={(event) => {
          if (event.key !== "Enter" || event.nativeEvent.isComposing || composing) return;
          event.preventDefault(); find.move(event.shiftKey ? -1 : 1);
        }} /></div>
      <span className="asb-session-find-status" role="status">{status}</span>
      <div className="asb-session-find-actions">
        <Tooltip label={previousLabel}>
          <Button variant="icon" className="asb-session-toolbar-button" disabled={moveDisabled}
            aria-label={previousLabel} onClick={() => find.move(-1)}><ChevronUp aria-hidden="true" /></Button>
        </Tooltip>
        <Tooltip label={nextLabel}>
          <Button variant="icon" className="asb-session-toolbar-button" disabled={moveDisabled}
            aria-label={nextLabel} onClick={() => find.move(1)}><ChevronDown aria-hidden="true" /></Button>
        </Tooltip>
        <Tooltip asChild label={closeLabel}>
          <Button variant="icon" aria-label={closeLabel} onClick={onClose}><X aria-hidden="true" /></Button>
        </Tooltip>
      </div>
    </div>
    {(error || (navigationFailed && result && result.page.total > 0)) && <div className="asb-session-find-error">
      {error && <p className="asb-warn-text" role="alert">{error}</p>}
      <Button variant="secondary" onClick={() => { onRetry(); find.reset(); input.current?.focus(); }}>
        {t("sessions.find.retry")}
      </Button>
    </div>}
    {result?.page.issues.map((issue) => <p key={issue} className="asb-warn-text" role="alert">{issue}</p>)}
  </div>;
}
