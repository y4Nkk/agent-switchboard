import { useCallback, useEffect, useRef, useState } from "react";

interface AutoQuery<T> {
  /** Latest successful result; null until the first one lands. */
  data: T | null;
  querying: boolean;
  /** Failure message of the newest request; cleared by the next run. */
  error: string | null;
  run: () => void;
}

/** The one owner of a panel's version-gated query: it reads on mount and on
 * key change, re-reads on the chosen minute interval, and only the newest
 * request's outcome lands. Concurrent reads are safe by version gating alone —
 * a run guard here would deadlock with StrictMode's remount cycle, whose
 * invalidated first read can never release it. */
export function useAutoQuery<T>(
  key: string,
  refreshIntervalMinutes: number,
  query: (key: string) => Promise<T>,
  fallbackError: string,
): AutoQuery<T> {
  const [data, setData] = useState<T | null>(null);
  const [querying, setQuerying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestVersion = useRef(0);

  const run = useCallback(async () => {
    const version = ++requestVersion.current;
    setQuerying(true);
    setError(null);
    try {
      const next = await query(key);
      if (requestVersion.current === version) setData(next);
    } catch (caught) {
      if (requestVersion.current === version) {
        setError((caught as { message?: string }).message ?? fallbackError);
      }
    } finally {
      if (requestVersion.current === version) setQuerying(false);
    }
  }, [fallbackError, key, query]);

  useEffect(() => {
    void run();
    return () => {
      requestVersion.current += 1;
    };
  }, [run]);

  useEffect(() => {
    if (refreshIntervalMinutes <= 0) return undefined;
    const timer = setInterval(() => void run(), refreshIntervalMinutes * 60_000);
    return () => clearInterval(timer);
  }, [refreshIntervalMinutes, run]);

  return { data, querying, error, run };
}
