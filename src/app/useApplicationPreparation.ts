import { useCallback, useEffect, useRef, useState } from "react";
import { getApplicationPreparation, recheckApplicationData, type ApplicationPreparation } from "../api/application-preparation";
import { onApplicationPreparationChanged, type CommandError } from "../api/client";
import { applyLanguagePreference } from "../i18n";
import { applyAppAppearance } from "../lib/app-appearance";
import { isBrowserDevelopment } from "../lib/runtime";

export function useApplicationPreparation() {
  const [snapshot, setSnapshot] = useState<ApplicationPreparation | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [activated, setActivated] = useState(false);
  const [completedRevision, setCompletedRevision] = useState(0);
  const revision = useRef(-1);
  const sessionId = useRef<string | null>(null);
  const mounted = useRef(false);
  const fetching = useRef(false);
  const queued = useRef(false);
  const refresh = useCallback(async function load() {
    if (fetching.current) { queued.current = true; return; }
    fetching.current = true;
    try {
      const next = await getApplicationPreparation();
      if (!mounted.current) return;
      if (next.sessionId !== sessionId.current) { sessionId.current = next.sessionId; revision.current = -1; }
      if (next.revision < revision.current) return;
      if (next.revision === revision.current) { setError(null); return; }
      if (next.settings) {
        applyLanguagePreference(next.settings.language);
        applyAppAppearance(next.settings);
      }
      revision.current = next.revision;
      setSnapshot(next); setError(null);
      if (next.status === "ready" || next.status === "degraded") {
        setActivated(true); setCompletedRevision((value) => value + 1);
      }
    } catch (caught) { if (mounted.current) setError(caught as CommandError); }
    finally {
      fetching.current = false;
      if (queued.current && mounted.current) { queued.current = false; void load(); }
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let stopped = false;
    let unlisten: (() => void) | undefined;
    void onApplicationPreparationChanged(() => void refresh()).then((stop) => {
      if (stopped) stop(); else unlisten = stop;
      if (!stopped) void refresh();
    }).catch((caught) => { if (!stopped) setError(caught as CommandError); });
    return () => { stopped = true; mounted.current = false; unlisten?.(); };
  }, [refresh]);
  useEffect(() => {
    if (snapshot?.status !== "running" && !isBrowserDevelopment) return;
    const timer = window.setInterval(() => void refresh(), isBrowserDevelopment ? 1000 : 300);
    return () => window.clearInterval(timer);
  }, [snapshot?.status, refresh]);
  const recheck = useCallback(async () => {
    try { await recheckApplicationData(); await refresh(); }
    catch (caught) { if (mounted.current) setError(caught as CommandError); }
  }, [refresh]);
  return { snapshot, error, activated, completedRevision, refresh, recheck };
}
