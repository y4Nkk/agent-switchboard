import { useEffect, useRef, useState } from "react";
import { useMessageState } from "../../i18n/use-message-state";

const waiting = new Set<() => Promise<void>>();
let reading = 0;

function drain() {
  while (reading < 2) {
    const job = waiting.values().next().value;
    if (!job) return;
    waiting.delete(job);
    reading += 1;
    void job().finally(() => { reading -= 1; drain(); });
  }
}

export function useReadingBlock<T>(read: () => Promise<T>, initial?: T) {
  const [value, setValue] = useState<T | null>(initial ?? null);
  const [busy, setBusy] = useState(initial === undefined);
  const [error, setError] = useMessageState();
  const [attempt, setAttempt] = useState(0);
  const seed = useRef(initial);
  useEffect(() => {
    if (seed.current !== undefined && attempt === 0) return;
    let active = true;
    setValue(null); setBusy(true); setError(null);
    const job = async () => {
      if (!active) return;
      try {
        const next = await read();
        if (active) setValue(next);
      } catch (caught) {
        if (active) setError(caught);
      } finally { if (active) setBusy(false); }
    };
    waiting.add(job);
    drain();
    return () => { active = false; waiting.delete(job); };
  }, [read, attempt, setError]);
  return { value, busy, error, retry: () => setAttempt((previous) => previous + 1) };
}
