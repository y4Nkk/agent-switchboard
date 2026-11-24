import { useCallback, useState } from "react";
import type { SessionMeta } from "../../api/client";
import { sessionKey } from "./session-content";

export function useSessionSelection() {
  const [chosen, setChosen] = useState(() => new Map<string, SessionMeta>());
  const choose = (sessions: readonly SessionMeta[], checked: boolean) => setChosen((current) => {
    const next = new Map(current);
    for (const session of sessions) {
      const key = sessionKey(session);
      if (checked) next.set(key, session);
      else next.delete(key);
    }
    return next;
  });
  const clear = useCallback(() => setChosen(new Map()), []);
  const remove = (removed: ReadonlySet<string>) => setChosen((current) =>
    new Map([...current].filter(([key]) => !removed.has(key))));
  const updateMetadata = (sessions: readonly SessionMeta[]) => setChosen((current) => {
    const next = new Map(current);
    for (const session of sessions) {
      const key = sessionKey(session);
      if (next.has(key)) next.set(key, session);
    }
    return next;
  });
  return { chosen, choose, clear, remove, updateMetadata };
}

export type SessionSelection = ReturnType<typeof useSessionSelection>;
