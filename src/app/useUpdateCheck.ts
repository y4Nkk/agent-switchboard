import { useCallback, useEffect, useRef, useState } from "react";
import { checkUpdate, getRuntimeOverview, getUpdateChannel,
  type CommandError, type UpdateChannel, type UpdateCheck } from "../api/client";
import { toast, toastMessage } from "../components/use-toast";
import { useUpdateDownload } from "./useUpdateDownload";

function useUpdateEnvironment(runCheck: (source: "manual" | "startup") => Promise<void>) {
  const [updateChannel, setUpdateChannel] = useState<UpdateChannel | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const automaticCheckStarted = useRef(false);
  useEffect(() => {
    let current = true;
    void getUpdateChannel().then((channel) => {
      if (!current) return;
      setUpdateChannel(channel);
      if (channel === "github" && !automaticCheckStarted.current) {
        automaticCheckStarted.current = true;
        void runCheck("startup");
      }
    }).catch(() => undefined);
    void getRuntimeOverview().then((overview) => {
      if (current) setAppVersion(overview.appVersion);
    }).catch(() => undefined);
    return () => { current = false; };
  }, [runCheck]);
  return { updateChannel, appVersion };
}

/** One app-wide state owner survives navigation away from About. */
export function useUpdateCheck({ onError }: { onError: (error: CommandError) => void }) {
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [lastCheckedAt, setLastCheckedAt] = useState<string | null>(null);
  const operation = useRef(false);
  const mounted = useRef(true);
  const onInstalled = useCallback(() => setUpdateCheck(null), []);
  const download = useUpdateDownload(operation, onInstalled, onError);
  const resetDownload = download.reset;
  const runUpdateCheck = useCallback(async (source: "manual" | "startup") => {
    if (operation.current || download.restartRequired) return;
    operation.current = true;
    setChecking(true);
    try {
      const next = await checkUpdate();
      if (!mounted.current) return;
      resetDownload();
      setUpdateCheck(next);
      setLastCheckedAt(next?.checkedAt ?? new Date().toISOString());
      if (source === "manual" && next === null) {
        toast({ kind: "success", title: toastMessage("backup.update.upToDate") });
      }
    } catch (error) {
      if (source === "manual" && mounted.current) onError(error as CommandError);
    } finally { operation.current = false; if (mounted.current) setChecking(false); }
  }, [operation, download.restartRequired, resetDownload, onError]);
  const environment = useUpdateEnvironment(runUpdateCheck);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  return { ...environment, updateCheck, checking, lastCheckedAt, ...download,
    runUpdateCheck: () => runUpdateCheck("manual"),
    installAvailableUpdate: () => { if (updateCheck) void download.start(updateCheck); },
    cancelUpdateDownload: download.cancel,
    restartInstalledUpdate: download.restart };
}
