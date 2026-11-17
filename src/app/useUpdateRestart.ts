import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { restartApplication, type CommandError, type UpdateDownloadState } from "../api/client";

export function useUpdateRestart(task: UpdateDownloadState | null, operation: RefObject<boolean>,
  mounted: RefObject<boolean>, onInstalled: () => void, onError: (error: CommandError) => void) {
  const installed = task?.phase === "installed";
  const [restarting, setRestarting] = useState(false);
  const [restartError, setRestartError] = useState<unknown>(null);
  const [restartConfirmationOpen, setRestartConfirmationOpen] = useState(false);
  const delivered = useRef<string | null>(null);
  const restartingNow = useRef(false);
  const confirmRestart = useCallback(async () => {
    if (!installed || !restartConfirmationOpen || restartingNow.current || !mounted.current) return;
    restartingNow.current = true;
    setRestarting(true);
    setRestartError(null);
    try { await restartApplication(); }
    catch (error) { if (mounted.current) { setRestartError(() => error); onError(error as CommandError); } }
    finally { restartingNow.current = false; if (mounted.current) setRestarting(false); }
  }, [installed, restartConfirmationOpen, mounted, onError]);
  useEffect(() => {
    if (!task || !["installed", "cancelled", "failed"].includes(task.phase) || delivered.current === task.id) return;
    delivered.current = task.id;
    operation.current = false;
    if (task.phase === "failed" && task.error) onError(task.error);
    if (task.phase === "installed") {
      onInstalled();
      setRestartConfirmationOpen(true);
    }
  }, [task, operation, onInstalled, onError]);
  const requestRestart = useCallback(() => {
    if (installed && !restartingNow.current) setRestartConfirmationOpen(true);
  }, [installed]);
  const dismissRestart = useCallback(() => {
    if (!restartingNow.current) setRestartConfirmationOpen(false);
  }, []);
  return { restarting, restartError, requestRestart, restartConfirmationOpen, confirmRestart, dismissRestart };
}
