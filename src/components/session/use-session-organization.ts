import { useCallback, useRef, useState } from "react";
import { updateSessionOrganization, type SessionMeta, type SessionOrganizationChange } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";
import { uiMessage } from "../../i18n/errors";

export const SESSION_ORGANIZATION_BATCH_LIMIT = 200;

export function useSessionOrganization(onSaved: (sessions: SessionMeta[]) => void) {
  const lock = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useMessageState();
  const [status, setStatus] = useMessageState();
  const resetFeedback = useCallback(() => { setError(null); setStatus(null); }, [setError, setStatus]);
  const save = async (sessions: SessionMeta[], change: SessionOrganizationChange) => {
    if (lock.current || sessions.length === 0) return;
    if (sessions.length > SESSION_ORGANIZATION_BATCH_LIMIT) { setError(uiMessage("sessions.organize.batchLimit")); return; }
    lock.current = true; setBusy(true); resetFeedback();
    try {
      const updated = await updateSessionOrganization(sessions.map(({ app, sessionId }) => ({ app, sessionId })), change);
      onSaved(updated);
      setStatus(uiMessage("sessions.organize.done", { count: updated.length }));
    } catch (caught) { setError(caught); }
    finally { lock.current = false; setBusy(false); }
  };
  return { save, busy, error, status, resetFeedback };
}

export type SessionOrganization = ReturnType<typeof useSessionOrganization>;
