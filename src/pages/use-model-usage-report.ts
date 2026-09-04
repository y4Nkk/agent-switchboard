import { useCallback, useEffect, useRef, useState } from "react";
import {
  getModelUsageReport,
  type ModelUsageRange,
  type ModelUsageRead,
} from "../api/client";

function errorMessage(caught: unknown): string {
  return (caught as { message?: string }).message ?? "无法汇总本地模型消耗";
}

function refreshDelay(refreshAfter: string): number {
  const target = new Date(refreshAfter).getTime();
  return Number.isNaN(target) ? 0 : Math.max(0, target - Date.now());
}

/** Renders the backend-owned local-session snapshot and revalidates it only
 * while this page is visible. Cache lifetime and persistence stay entirely in
 * the desktop service; the renderer only follows its returned deadline. */
export function useModelUsageReport(active: boolean, range: ModelUsageRange) {
  const [displayRange, setDisplayRange] = useState(range);
  const [read, setRead] = useState<ModelUsageRead | null>(null);
  const [refreshPlan, setRefreshPlan] = useState<{
    range: ModelUsageRange;
    refreshAfter: string;
  } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestVersion = useRef(0);

  const load = useCallback(async (forceRefresh: boolean): Promise<ModelUsageRead | null> => {
    if (!active) return null;

    const version = ++requestVersion.current;
    setDisplayRange(range);
    setError(null);
    setLoading(true);
    try {
      const next = await getModelUsageReport({ range, forceRefresh });
      if (requestVersion.current === version) {
        setRead(next);
        setRefreshPlan({ range, refreshAfter: next.refreshAfter });
      }
      return next;
    } catch (caught) {
      if (requestVersion.current === version) setError(errorMessage(caught));
      return null;
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }, [active, range]);

  const refresh = useCallback(() => load(true), [load]);

  useEffect(() => {
    if (!active) {
      requestVersion.current += 1;
      setLoading(false);
      return undefined;
    }

    void load(false);

    return () => {
      requestVersion.current += 1;
    };
  }, [active, load, range]);

  useEffect(() => {
    if (!active || refreshPlan?.range !== range) return undefined;

    const timer = setTimeout(() => {
      void load(true);
    }, refreshDelay(refreshPlan.refreshAfter));

    return () => clearTimeout(timer);
  }, [active, load, range, refreshPlan]);

  const isCurrentRange = displayRange === range;

  return {
    read: isCurrentRange ? read : null,
    loading: isCurrentRange ? loading : false,
    error: isCurrentRange ? error : null,
    refresh,
  };
}
