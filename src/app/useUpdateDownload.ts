import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { cancelUpdateDownload, readUpdateDownload, startUpdateDownload,
  type CommandError, type UpdateCheck, type UpdateDownloadState } from "../api/client";
import { observeUpdateDownload } from "./update-download-reader";
import { latestDownloadState, updateDownloadActive } from "./update-download-state";
import { useUpdateRestart } from "./useUpdateRestart";

export function useUpdateDownload(operation: RefObject<boolean>, onInstalled: () => void,
  onError: (error: CommandError) => void) {
  const [task, setTask] = useState<UpdateDownloadState | null>(null);
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [startError, setStartError] = useState<CommandError | null>(null);
  const [readError, setReadError] = useState<unknown>(null);
  const [cancelError, setCancelError] = useState<unknown>(null);
  const mounted = useRef(true);
  const taskId = useRef<string | null>(null);
  const cancelRequested = useRef(false);
  const restart = useUpdateRestart(task, operation, mounted, onInstalled, onError);
  const accept = useCallback((next: UpdateDownloadState) => {
    if (mounted.current && taskId.current === next.id) setTask((current) => latestDownloadState(current, next));
  }, []);
  const sendCancel = useCallback(async (id: string) => {
    setCancelling(true);
    setCancelError(null);
    try { accept(await cancelUpdateDownload(id)); }
    catch (error) {
      if (mounted.current && taskId.current === id) { setCancelError(() => error); onError(error as CommandError); }
    }
    finally { if (mounted.current && taskId.current === id) setCancelling(false); }
  }, [accept, onError]);
  const start = useCallback(async (update: UpdateCheck) => {
    if (operation.current || restart.restartRequired) return;
    operation.current = true;
    cancelRequested.current = false;
    taskId.current = null;
    setStarting(true); setCancelling(false); setTask(null); setReadError(null); setStartError(null); setCancelError(null);
    try {
      const initial = await startUpdateDownload(update.id);
      if (!mounted.current) { void cancelUpdateDownload(initial.id).catch(console.error); return; }
      taskId.current = initial.id;
      accept(initial);
      if (cancelRequested.current) void sendCancel(initial.id).finally(() => {
        if (taskId.current === initial.id) cancelRequested.current = false;
      });
    } catch (error) {
      operation.current = false;
      cancelRequested.current = false;
      if (mounted.current) { setCancelling(false); setStartError(error as CommandError); onError(error as CommandError); }
    } finally { if (mounted.current) setStarting(false); }
  }, [operation, restart.restartRequired, accept, sendCancel, setReadError, onError]);
  const cancel = useCallback(() => {
    if ((!task && !starting) || (task && !["downloading", "verifying"].includes(task.phase))) return;
    if (cancelRequested.current) return;
    cancelRequested.current = true;
    const id = taskId.current;
    if (id) void sendCancel(id).finally(() => {
      if (taskId.current === id) cancelRequested.current = false;
    });
    else setCancelling(true);
  }, [sendCancel, task, starting]);
  const active = task !== null && updateDownloadActive(task.phase);
  const reset = useCallback(() => {
    taskId.current = null;
    cancelRequested.current = false;
    setTask(null); setReadError(null); setCancelling(false); setStartError(null); setCancelError(null);
  }, [setReadError]);
  useEffect(() => {
    if (!taskId.current || !active) return;
    return observeUpdateDownload(taskId.current, { read: readUpdateDownload, onRead: accept,
      onError: (error) => setReadError(() => error) });
  }, [task?.id, active, accept, setReadError]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      cancelRequested.current = true;
      if (taskId.current) void cancelUpdateDownload(taskId.current).catch(console.error);
    };
  }, []);
  return { state: { task, starting, cancelling, startError, readError, cancelError,
    restartRequired: restart.restartRequired, restarting: restart.restarting, restartError: restart.restartError },
    start, cancel, reset, restart: restart.restart };
}
