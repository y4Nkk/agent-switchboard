import { useCallback, useEffect, useRef, useState } from "react";
import { getSidebarCollapsed, setSidebarCollapsed, type CommandError } from "../api/client";

/** Publishes only saved layout state; a failed read remains unknown until retried. */
export function useSidebarState(onError: (error: CommandError) => void) {
  const [collapsed, setCollapsed] = useState<boolean | null>(null);
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState(true);
  const inFlight = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    let disposed = false;
    mounted.current = true;
    void getSidebarCollapsed().then((saved) => {
      if (!disposed) setCollapsed(saved);
    }).catch((error: CommandError) => {
      if (!disposed) onError(error);
    }).finally(() => {
      if (!disposed) { setReady(true); setPending(false); }
    });
    return () => { disposed = true; mounted.current = false; };
  }, [onError]);
  const onToggle = useCallback(async () => {
    if (pending || inFlight.current) return;
    inFlight.current = true;
    setPending(true);
    try {
      const saved = collapsed === null
        ? await getSidebarCollapsed()
        : await setSidebarCollapsed(!collapsed);
      if (mounted.current) setCollapsed(saved);
    } catch (error) {
      if (mounted.current) onError(error as CommandError);
    } finally {
      inFlight.current = false;
      if (mounted.current) setPending(false);
    }
  }, [collapsed, pending, onError]);
  return { collapsed, ready, pending, onToggle };
}
