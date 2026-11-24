import { useCallback, useEffect, useRef, useState } from "react";
import { hideTray, onTrayHidden, switchTrayProvider, type TraySnapshot } from "../api/client";
import { queryCodexOfficialQuota, queryProfileUsage } from "../api/usage";
import { useMessageState } from "../i18n/use-message-state";
import { ARMED_SWITCH_TTL_MS, switchClickOutcome, type ArmedSwitch } from "./tray-switch";

type PendingAction = { kind: "switch" | "refresh"; providerId: string } | { kind: "panel" };

/** All tray commands share one serial action and structured error owner. */
export function useTrayActions(refresh: () => Promise<void>) {
  const [error, setError] = useMessageState();
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [armed, setArmed] = useState<ArmedSwitch | null>(null);
  const actionInFlight = useRef(false);
  const armedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mounted = useRef(true);
  const disarm = useCallback(() => {
    if (armedTimer.current !== null) { clearTimeout(armedTimer.current); armedTimer.current = null; }
    setArmed(null);
  }, []);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let stop: (() => void) | undefined;
    void onTrayHidden(disarm).then((unlisten) => { if (disposed) unlisten(); else stop = unlisten; })
      .catch((caught) => { if (!disposed) setError(caught); });
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        disarm();
        void hideTray().catch((caught) => { if (mounted.current) setError(caught); });
      }
    };
    window.addEventListener("blur", disarm);
    document.addEventListener("keydown", onKey);
    return () => {
      mounted.current = false;
      disposed = true;
      stop?.();
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", disarm);
      if (armedTimer.current !== null) clearTimeout(armedTimer.current);
    };
  }, [setError, disarm]);
  const run = async (mark: PendingAction, action: () => Promise<void>) => {
    if (actionInFlight.current) return;
    actionInFlight.current = true;
    setPending(mark);
    setError(null);
    try { await action(); }
    catch (caught) { if (mounted.current) setError(caught); }
    finally {
      actionInFlight.current = false;
      if (mounted.current) setPending(null);
    }
  };
  /** The first click on a row only marks it; the confirming click switches. */
  const requestSwitch = (providerId: string) => {
    if (actionInFlight.current) return;
    const now = performance.now();
    if (switchClickOutcome(armed, providerId, now) === "arm") {
      if (armedTimer.current !== null) clearTimeout(armedTimer.current);
      armedTimer.current = setTimeout(disarm, ARMED_SWITCH_TTL_MS);
      setArmed({ providerId, expiresAt: now + ARMED_SWITCH_TTL_MS });
      return;
    }
    disarm();
    void run({ kind: "switch", providerId }, async () => {
      await switchTrayProvider(providerId);
      await refresh();
    });
  };
  const refreshUsage = (provider: TraySnapshot["providers"][number]) => {
    void run({ kind: "refresh", providerId: provider.id }, async () => {
      if (provider.usage?.kind === "official") await queryCodexOfficialQuota(provider.id);
      else await queryProfileUsage(provider.app, provider.id);
      await refresh();
    });
  };
  return { error, setError, pending, armedId: armed?.providerId ?? null, run, requestSwitch, refreshUsage };
}
