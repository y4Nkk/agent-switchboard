import { useRef, useState } from "react";
import { deleteSessions, type SessionMeta } from "../../api/client";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { ConfirmSheet } from "../ConfirmSheet";
import { toast, toastMessage } from "../use-toast";
import { ToastMessageList } from "../ToastContent";
import { sessionKey, sessionTitle } from "./session-content";

export function useSessionDeletion(onRemoved: (keys: Set<string>) => void) {
  const [selecting, setSelecting] = useState(false);
  const [chosen, setChosen] = useState(new Map<string, SessionMeta>());
  const [pending, setPending] = useState<SessionMeta[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const lock = useRef(false);
  const toggle = (session: SessionMeta) => setChosen((current) => {
    const next = new Map(current);
    if (next.has(sessionKey(session))) next.delete(sessionKey(session));
    else next.set(sessionKey(session), session);
    return next;
  });
  const toggleMode = () => { setSelecting(!selecting); setChosen(new Map()); };
  const choosePage = (sessions: SessionMeta[]) => setChosen((current) => {
    const next = new Map(current);
    sessions.forEach((session) => next.set(sessionKey(session), session));
    return next;
  });
  const confirm = async () => {
    if (lock.current || pending.length === 0) return;
    lock.current = true; setBusy(true); setError(null);
    const targets = pending;
    try {
      const outcomes = await deleteSessions(targets.map(({ app, sessionId }) => ({ app, sessionId })));
      const removed = new Set(outcomes.filter((item) => item.deleted).map(sessionKey));
      setChosen((current) => new Map([...current].filter(([key]) => !removed.has(key))));
      onRemoved(removed);
      if (removed.size) toast({ kind: "success", title: toastMessage("sessions.batchDelete.done", { count: removed.size }) });
      const failed = outcomes.filter((item) => !item.deleted || item.error);
      if (failed.length) toast({ kind: "error", title: toastMessage(failed.some((item) => item.deleted)
        ? "sessions.batchDelete.warningTitle" : "sessions.batchDelete.failedTitle", { count: failed.length }),
        description: <ToastMessageList items={failed.map((item) => {
          const target = targets.find((target) => sessionKey(target) === sessionKey(item));
          const title = target ? sessionTitle(target) : item.sessionId;
          return item.error ? toastMessage("sessions.batchDelete.failedItem", { title, reason: item.error })
            : toastMessage("sessions.batchDelete.failedUnknown", { title });
        })} /> });
    } catch (caught) { setError(caught); }
    finally { lock.current = false; setBusy(false); setPending([]); }
  };
  const updateMetadata = (sessions: SessionMeta[]) => setChosen((current) => new Map([...current].map(([key, value]) =>
    [key, sessions.find((session) => sessionKey(session) === key) ?? value])));
  return { selecting, chosen, pending, busy, error, toggle, toggleMode, choosePage, setPending, confirm, updateMetadata };
}

export type SessionDeletionState = ReturnType<typeof useSessionDeletion>;

export function SessionDeletion({ state, disabled = false }: { state: SessionDeletionState; disabled?: boolean }) {
  const { t } = useI18n();
  return <>
    {state.error && <p className="asb-warn-text" role="alert">{state.error}</p>}
    {state.pending.length > 0 && <ConfirmSheet
      title={t(state.pending.length === 1 ? "sessions.delete.title" : "sessions.batchDelete.title")}
      confirmLabel={t(state.pending.length === 1 ? "sessions.delete.confirm" : "sessions.batchDelete.confirm")}
      confirmDisabled={state.busy || disabled}
      busy={state.busy}
      destructive onConfirm={() => void state.confirm()}
      onCancel={() => state.setPending([])}>
      <ul className="asb-dialog-details">
        <li>{t("sessions.batchDelete.detail.count", { count: state.pending.length })}</li>
        {state.pending.slice(0, 3).map((item) => <li key={sessionKey(item)}>{sessionTitle(item)}</li>)}
        {state.pending.length > 3 && <li>{t("sessions.batchDelete.detail.more", { count: state.pending.length - 3 })}</li>}
        <li>{t(state.pending.length === 1 ? "sessions.delete.detail" : "sessions.batchDelete.detail.individual")}</li>
        {state.pending.some((item) => item.app === "codex") && <li>{t("sessions.delete.codexDetail")}</li>}
        <li>{t("sessions.bookmarks.retained")}</li>
      </ul>
    </ConfirmSheet>}
  </>;
}
