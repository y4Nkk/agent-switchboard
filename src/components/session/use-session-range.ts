import { useEffect, useState } from "react";
import { discardSessionRange, executeSessionRange, previewSessionRange, readSessionRangeProgress,
  type SessionRangeAction, type SessionRangePreview, type SessionRangeProgress } from "../../api/session-range";
import type { AppKind } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import { dateBoundary, localDate } from "./session-dates";

export function useSessionRange(app: AppKind | null, archived: boolean, initial: { from: string; to: string },
  onChanged: () => void, onClose: () => void) {
  const [dates, setDates] = useState(() => {
    const today = new Date(); const from = new Date(); from.setDate(today.getDate() - 30);
    return { from: initial.from || localDate(from), to: initial.to || localDate(today) };
  });
  const [action, setAction] = useState<SessionRangeAction>(archived ? "unarchive" : "archive");
  const [preview, setPreview] = useState<SessionRangePreview | null>(null);
  const [progress, setProgress] = useState<SessionRangeProgress | null>(null);
  const [resultAction, setResultAction] = useState<SessionRangeAction>(action);
  const [stage, setStage] = useState<"preview" | "execute" | "close" | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useMessageState();
  const busy = stage !== null;
  const after = dateBoundary(dates.from, false), before = dateBoundary(dates.to, true);
  const valid = Boolean(after && before && dates.from <= dates.to);
  useEffect(() => {
    if (stage !== "execute" || !preview) return;
    let stopped = false; let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { const next = await readSessionRangeProgress(preview.id); if (!stopped && next?.running) setProgress(next); }
      catch (caught) { if (!stopped) setError(caught); }
      if (!stopped) timer = setTimeout(() => void poll(), 500);
    };
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  }, [stage, preview, setError]);
  const prepare = async () => {
    if (busy || !valid || !after || !before) return;
    setStage("preview"); setError(null); setProgress(null); setPreview(null); setConfirmed(false);
    try { setPreview(await previewSessionRange({ app, archived, activeAfter: after, activeBefore: before }, action)); }
    catch (caught) { setError(caught); }
    finally { setStage(null); }
  };
  const execute = async () => {
    if (busy || !confirmed || !preview) return;
    setStage("execute"); setError(null); setResultAction(preview.action);
    setProgress({ id: preview.id, running: true, completed: 0, total: preview.items.length, outcomes: [], warning: null });
    try { setProgress(await executeSessionRange(preview.id, confirmed)); }
    catch (caught) {
      setError(caught);
      try { const latest = await readSessionRangeProgress(preview.id); setProgress(latest?.completed ? latest : null); }
      catch { setProgress(null); }
    }
    finally { setStage(null); setPreview(null); setConfirmed(false); onChanged(); }
  };
  const edit = async () => {
    if (busy) return;
    setStage("close");
    try { if (preview) await discardSessionRange(preview.id); setPreview(null); setProgress(null); setConfirmed(false); }
    catch (caught) { setError(caught); }
    finally { setStage(null); }
  };
  const close = async () => {
    if (busy) return;
    setStage("close");
    try { if (preview) await discardSessionRange(preview.id); onClose(); }
    catch (caught) { setError(caught); setStage(null); }
  };
  return { dates, setDates, action, setAction, preview, progress, resultAction, stage, busy, confirmed, setConfirmed, error,
    valid, prepare, execute, edit, close };
}

export type SessionRangeState = ReturnType<typeof useSessionRange>;
