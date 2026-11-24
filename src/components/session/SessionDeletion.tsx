import { useCallback, useRef, useState } from "react";
import { deleteSessions, type SessionMeta } from "../../api/client";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { ConfirmSheet } from "../ConfirmSheet";
import { toast, toastMessage, type ToastMessage } from "../use-toast";
import { ToastMessageList } from "../ToastContent";
import { sessionKey, sessionTitle } from "./session-content";

export function useSessionDeletion(onRemoved: (keys: Set<string>) => void) {
  const [pending, setPending] = useState<SessionMeta[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const [feedback, setFeedback] = useState<{
    deleted: ToastMessage | null;
    failed: { title: ToastMessage; items: ToastMessage[] } | null;
  } | null>(null);
  const lock = useRef(false);
  const reset = useCallback(() => {
    setPending([]); setError(null); setFeedback(null);
  }, [setError]);
  const confirm = async () => {
    if (lock.current || pending.length === 0) return;
    lock.current = true; setBusy(true); setError(null); setFeedback(null);
    const targets = pending;
    try {
      const outcomes = await deleteSessions(targets.map(({ app, sessionId }) => ({ app, sessionId })));
      const removed = new Set(outcomes.filter((item) => item.deleted).map(sessionKey));
      onRemoved(removed);
      const deleted = removed.size ? toastMessage("sessions.batchDelete.done", { count: removed.size }) : null;
      const issues = outcomes.filter((item) => !item.deleted || item.error);
      const failed = issues.length ? {
        title: toastMessage(issues.some((item) => item.deleted) ? "sessions.batchDelete.warningTitle"
          : "sessions.batchDelete.failedTitle", { count: issues.length }),
        items: issues.map((item) => {
          const target = targets.find((target) => sessionKey(target) === sessionKey(item));
          const title = target ? sessionTitle(target) : item.sessionId;
          return item.error ? toastMessage("sessions.batchDelete.failedItem", { title, reason: item.error })
            : toastMessage("sessions.batchDelete.failedUnknown", { title });
        }),
      } : null;
      setFeedback({ deleted, failed });
      if (deleted) toast({ kind: "success", title: deleted });
      if (failed) toast({ kind: "error", title: failed.title, description: <ToastMessageList items={failed.items} /> });
    } catch (caught) { setError(caught); }
    finally { lock.current = false; setBusy(false); setPending([]); }
  };
  return { pending, busy, error, feedback, reset, setPending, confirm };
}

export type SessionDeletionState = ReturnType<typeof useSessionDeletion>;

export function SessionDeletionConfirm({ state, disabled }: { state: SessionDeletionState; disabled: boolean }) {
  const { t } = useI18n();
  if (state.pending.length === 0) return null;
  return <ConfirmSheet
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
  </ConfirmSheet>;
}
