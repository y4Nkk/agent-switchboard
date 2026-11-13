import { useCallback, useEffect, useRef, useState } from "react";
import { checkUpdate, type CommandError, type UpdateCheck } from "../api/client";
import { toast, toastMessage } from "../components/use-toast";
import { applicationUpdateView, type ApplicationUpdateControl, type ApplicationUpdateSnapshot } from "./update-state";
import { useUpdateDownload } from "./useUpdateDownload";
import { useUpdateEnvironment } from "./useUpdateEnvironment";

/** One application owner survives navigation and preserves command diagnostics. */
export function useApplicationUpdate({ onError }: { onError: (error: CommandError) => void }): ApplicationUpdateControl {
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [lastCheckedAt, setLastCheckedAt] = useState<string | null>(null);
  const [checkError, setCheckError] = useState<unknown>(null);
  const operation = useRef(false);
  const mounted = useRef(true);
  const automaticCheckStarted = useRef(false);
  const { readChannel, ...environment } = useUpdateEnvironment();
  const onInstalled = useCallback(() => setUpdateCheck(null), []);
  const download = useUpdateDownload(operation, onInstalled, onError);
  const resetDownload = download.reset;
  const runCheck = useCallback(async (source: "manual" | "startup") => {
    if (environment.updateChannel !== "github" || operation.current || download.state.restartRequired) return;
    operation.current = true;
    setChecking(true); setCheckError(null); resetDownload();
    try {
      const next = await checkUpdate();
      if (!mounted.current) return;
      setUpdateCheck(next);
      setLastCheckedAt(next?.checkedAt ?? new Date().toISOString());
      if (source === "manual" && next === null) toast({ kind: "success", title: toastMessage("backup.update.upToDate") });
    } catch (error) {
      if (mounted.current) { setCheckError(() => error); if (source === "manual") onError(error as CommandError); }
    } finally { operation.current = false; if (mounted.current) setChecking(false); }
  }, [environment.updateChannel, download.state.restartRequired, resetDownload, onError]);
  useEffect(() => {
    if (environment.updateChannel === "github" && !automaticCheckStarted.current) {
      automaticCheckStarted.current = true;
      void runCheck("startup");
    }
  }, [environment.updateChannel, runCheck]);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const state: ApplicationUpdateSnapshot = { ...environment, updateCheck, checking, checkError, lastCheckedAt, ...download.state };
  const view = applicationUpdateView(state);
  return { state,
    onCheck: () => {
      if (state.updateChannel === null) void readChannel();
      else if (view.canCheck) void runCheck("manual");
    },
    onInstall: () => {
      if (view.canInstall && updateCheck && !operation.current) { setCheckError(null); void download.start(updateCheck); }
    },
    onCancel: download.cancel, onRestart: download.restart };
}
