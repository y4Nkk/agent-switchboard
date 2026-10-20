import { useRef, useState } from "react";
import { convertSessionSources, type SessionConversionOutcome } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";

export function useSessionConversion(onConverted: () => void) {
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<SessionConversionOutcome | null>(null);
  const [error, setError] = useMessageState();
  const running = useRef(false);
  const convert = async (ids: string[]) => {
    if (running.current || ids.length === 0) return;
    running.current = true;
    setBusy(true); setError(null); setOutcome(null);
    try {
      const result = await convertSessionSources(ids);
      setOutcome(result);
      onConverted();
    } catch (caught) { setError(caught); }
    finally { running.current = false; setBusy(false); }
  };
  return { busy, outcome, error, convert };
}

export type SessionConversion = ReturnType<typeof useSessionConversion>;
