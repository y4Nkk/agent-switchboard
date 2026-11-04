import { useCallback, useEffect, useRef, useState } from "react";
import { getCodexProbeBatch, type CodexProbeBatch } from "../api/client";
import { useReadQueue } from "../hooks/use-read-queue";
import { uiMessage } from "../i18n/errors";
import { useMessageState } from "../i18n/use-message-state";
import { startProbeDetailReading } from "./probe-detail-reader";

export function useCodexProbeDetail(active: boolean, batchId: string | null, revision: string, retain: boolean) {
  const [snapshot, setSnapshot] = useState<{ id: string; batch: CodexProbeBatch } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useMessageState();
  const enqueue = useReadQueue();
  const reader = useRef<ReturnType<typeof startProbeDetailReading> | null>(null);
  useEffect(() => {
    if (!active || batchId === null) {
      if (!retain || batchId === null) setSnapshot(null);
      setLoading(false);
      return;
    }
    setSnapshot((current) => current?.id === batchId ? current : null);
    setError(null);
    const lifetime = startProbeDetailReading({ batchId, read: getCodexProbeBatch, enqueue,
      onLoading: setLoading, onError: setError, onRead: (batch) => {
        setSnapshot(batch ? { id: batchId, batch } : null);
        setError(batch ? null : uiMessage("codex.history.missingBatch"));
      } });
    reader.current = lifetime;
    return () => { lifetime.stop(); reader.current = null; };
  }, [active, batchId, revision, retain, enqueue, setError]);
  const refresh = useCallback(() => reader.current?.refresh(), []);
  return { detail: (active || retain) && snapshot?.id === batchId ? snapshot.batch : null,
    detailLoading: active && loading, detailError: active ? error : null, refreshDetail: refresh };
}
