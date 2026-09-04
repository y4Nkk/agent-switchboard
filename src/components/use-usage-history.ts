import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getUsageHistory,
  type UsageHistoryRequest,
  type UsageHistorySeries,
} from "../api/client";

function requestKey(request: UsageHistoryRequest): string {
  return request.kind === "provider" ? `provider:${request.profileId}` : "official";
}

function errorMessage(reason: unknown): string {
  return (reason as { message?: string }).message ?? "无法读取用量历史";
}

/** Owns a rendered panel's read-only history request. A completed live query
 * calls `refresh` to show its newly persisted point without any second
 * provider request. */
export function useUsageHistory(request: UsageHistoryRequest, revalidationKey = "") {
  const key = requestKey(request);
  const currentRequest = useMemo<UsageHistoryRequest>(
    () => (request.kind === "provider" ? { kind: "provider", profileId: request.profileId } : { kind: "official" }),
    [key],
  );
  const [series, setSeries] = useState<UsageHistorySeries[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const version = useRef(0);

  const refresh = useCallback(async () => {
    const current = ++version.current;
    setLoading(true);
    setError(null);
    try {
      const next = await getUsageHistory(currentRequest);
      if (!Array.isArray(next)) throw new Error("用量历史响应格式无效");
      if (version.current === current) setSeries(next);
    } catch (reason) {
      if (version.current === current) setError(errorMessage(reason));
    } finally {
      if (version.current === current) setLoading(false);
    }
  }, [currentRequest]);

  useEffect(() => {
    setSeries([]);
    setError(null);
    void refresh();
    return () => {
      version.current += 1;
    };
  }, [refresh, revalidationKey]);

  return { series, loading, error, refresh };
}
