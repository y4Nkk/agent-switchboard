import { useCallback, useEffect, useRef, useState } from "react";
import { queryProfileUsage } from "../api/client";
import { refreshProviderQuotas, type QuotaProvider } from "../lib/quota-profiles";

export function useProviderQuotaRefresh(active: boolean, profiles: readonly QuotaProvider[]) {
  const [loading, setLoading] = useState(false);
  const inflight = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const refresh = useCallback(async () => {
    if (!active || inflight.current) return;
    inflight.current = true;
    setLoading(true);
    try {
      await refreshProviderQuotas(profiles, queryProfileUsage);
    } finally {
      inflight.current = false;
      if (mounted.current) setLoading(false);
    }
  }, [active, profiles]);
  return { loading, refresh, enabled: active && profiles.some((profile) => profile.usageQuery !== null) };
}
