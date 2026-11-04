import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { readModelUsageSnapshot, refreshModelUsage, readModelUsageRefresh, type ModelUsageRange, type ModelUsageRead } from "../api/client";
import { startModelUsageReading, type ModelUsageActivity, type ModelUsageRunning } from "./model-usage-reader";

export function useModelUsageReport(active: boolean, range: ModelUsageRange) {
  const [snapshot, setSnapshot] = useState<ModelUsageRead | null>(null);
  const [activity, setActivity] = useState<ModelUsageActivity>(null);
  const [task, setTask] = useState<ModelUsageRunning | null>(null);
  const [error, setError] = useMessageState();
  const reader = useRef<ReturnType<typeof startModelUsageReading> | null>(null);

  useEffect(() => {
    if (!active) return undefined;
    const lifetime = startModelUsageReading({ readSnapshot: readModelUsageSnapshot, refresh: refreshModelUsage,
      readRefresh: readModelUsageRefresh, onProgress: setTask,
      onActivity: setActivity, onError: setError, onRead: setSnapshot });
    reader.current = lifetime;
    return () => { lifetime.stop(); reader.current = null; };
  }, [active, setError]);

  const refresh = useCallback(() => { void reader.current?.refresh(); }, []);
  const read = useMemo(() => {
    const report = snapshot?.reports.find((candidate) => candidate.range === range);
    return snapshot && report ? { report, freshness: snapshot.freshness, refreshAfter: snapshot.refreshAfter } : null;
  }, [snapshot, range]);
  return { read, task: active ? task : null, activity: active ? activity : null,
    loading: active && activity !== null, error, refresh };
}
