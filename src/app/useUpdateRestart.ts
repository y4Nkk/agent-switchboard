import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { restartApplication, type CommandError, type UpdateDownloadState } from "../api/client";

export function useUpdateRestart(task: UpdateDownloadState | null, operation: RefObject<boolean>,
  mounted: RefObject<boolean>, onInstalled: () => void, onError: (error: CommandError) => void) {
  const [restartRequired, setRestartRequired] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [restartError, setRestartError] = useState<unknown>(null);
  const delivered = useRef<string | null>(null);
  const restartingNow = useRef(false);
  const relaunch = useCallback(async () => {
    if (restartingNow.current) return;
    restartingNow.current = true;
    setRestarting(true);
    setRestartError(null);
    try { await restartApplication(); }
    catch (error) { if (mounted.current) { setRestartError(() => error); onError(error as CommandError); } }
    finally { restartingNow.current = false; if (mounted.current) setRestarting(false); }
  }, [mounted, onError]);
  useEffect(() => {
    if (!task || !["installed", "cancelled", "failed"].includes(task.phase) || delivered.current === task.id) return;
    delivered.current = task.id;
    operation.current = false;
    if (task.phase === "failed" && task.error) onError(task.error);
    if (task.phase === "installed") {
      setRestartRequired(true);
      onInstalled();
      void relaunch();
    }
  }, [task, operation, onInstalled, onError, relaunch]);
  const restart = useCallback(() => { if (restartRequired) void relaunch(); }, [restartRequired, relaunch]);
  return { restartRequired, restarting, restartError, restart };
}
