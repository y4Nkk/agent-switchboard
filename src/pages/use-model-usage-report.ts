import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useRef, useState } from "react";
import { useReadQueue } from "../hooks/use-read-queue";
import { getModelUsageReport, type ModelUsageRange, type ModelUsageRead } from "../api/client";

function refreshDelay(refreshAfter: string): number {
  const target = new Date(refreshAfter).getTime();
  return Number.isNaN(target) ? 30_000 : Math.max(0, target - Date.now());
}

/** One visible reader owns entry, manual refresh, retries and the next deadline. */
export function useModelUsageReport(active: boolean, range: ModelUsageRange) {
  const [read, setRead] = useState<ModelUsageRead | null>(null);
  const [displayRange, setDisplayRange] = useState(range);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useMessageState();
  const [revision, setRevision] = useState(0);
  const requestedRefresh = useRef(false);
  const enqueue = useReadQueue();
  const refresh = useCallback(() => { requestedRefresh.current = true; setRevision((value) => value + 1); }, []);

  useEffect(() => {
    if (!active) { setLoading(false); return undefined; }
    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cancel: (() => void) | undefined;
    const load = (forceRefresh: boolean) => {
      if (!current) return;
      setDisplayRange(range); setLoading(true); setError(null);
      cancel = enqueue(async () => {
        if (!current) return;
        try {
          const next = await getModelUsageReport({ range, forceRefresh });
          if (!current) return;
          setRead(next);
          timer = setTimeout(() => load(false), refreshDelay(next.refreshAfter));
        } catch (caught) {
          if (!current) return;
          setError(caught);
          timer = setTimeout(() => load(false), 30_000);
        } finally { if (current) setLoading(false); }
      }, (caught) => { if (current) setError(caught); });
    };
    const forceRefresh = requestedRefresh.current;
    requestedRefresh.current = false;
    load(forceRefresh);
    return () => { current = false; cancel?.(); clearTimeout(timer); };
  }, [active, range, revision, enqueue, setError]);

  return { read: read?.report.range === range ? read : null,
    loading: displayRange === range ? loading : active,
    error: displayRange === range ? error : null, refresh };
}
