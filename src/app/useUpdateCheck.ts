import { useCallback, useEffect, useRef, useState } from "react";
import {
  checkUpdate,
  closeUpdate,
  installUpdate,
  restartApplication,
  type CommandError,
  type UpdateCheck,
} from "../api/client";

export interface UpdateDownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null;
}

interface UpdateCheckDeps {
  onError: (error: CommandError) => void;
}

/** The sole frontend state owner for signed update checks and installation. */
export function useUpdateCheck({ onError }: UpdateCheckDeps) {
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<UpdateDownloadProgress | null>(null);
  const [lastCheckedAt, setLastCheckedAt] = useState<string | null>(null);
  const [restartRequired, setRestartRequired] = useState(false);
  const activeUpdate = useRef<UpdateCheck | null>(null);
  const disposed = useRef(false);
  const inFlight = useRef(false);
  const automaticCheckStarted = useRef(false);

  const discardUpdate = useCallback((update: UpdateCheck | null) => {
    if (update) void closeUpdate(update.update).catch(() => undefined);
  }, []);

  const replaceUpdate = useCallback(
    (next: UpdateCheck | null) => {
      const previous = activeUpdate.current;
      activeUpdate.current = next;
      if (!disposed.current) setUpdateCheck(next);
      if (previous && previous !== next) discardUpdate(previous);
    },
    [discardUpdate],
  );

  const runUpdateCheck = useCallback(async (reportFailure: boolean) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setChecking(true);
    try {
      const next = await checkUpdate();
      if (disposed.current) {
        discardUpdate(next);
        return;
      }
      replaceUpdate(next);
      setLastCheckedAt(next?.checkedAt ?? new Date().toISOString());
    } catch (caught) {
      if (reportFailure) onError(caught as CommandError);
    } finally {
      inFlight.current = false;
      if (!disposed.current) setChecking(false);
    }
  }, [discardUpdate, onError, replaceUpdate]);

  useEffect(() => {
    disposed.current = false;
    return () => {
      disposed.current = true;
      discardUpdate(activeUpdate.current);
      activeUpdate.current = null;
    };
  }, [discardUpdate]);

  useEffect(() => {
    if (automaticCheckStarted.current) return;
    automaticCheckStarted.current = true;
    void runUpdateCheck(false);
  }, [runUpdateCheck]);

  const installAvailableUpdate = useCallback(async () => {
    const availableUpdate = activeUpdate.current;
    if (!availableUpdate || installing || restartRequired) return;
    setInstalling(true);
    setDownloadProgress(null);
    let installed = false;
    try {
      await installUpdate(availableUpdate.update, (event) => {
        if (event.event === "Started") {
          setDownloadProgress({ downloadedBytes: 0, totalBytes: event.data.contentLength ?? null });
        } else if (event.event === "Progress") {
          setDownloadProgress((current) => ({
            downloadedBytes: (current?.downloadedBytes ?? 0) + event.data.chunkLength,
            totalBytes: current?.totalBytes ?? null,
          }));
        }
      });
      installed = true;
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      replaceUpdate(null);
    }
    if (!installed) {
      setInstalling(false);
      return;
    }
    setRestartRequired(true);
    try {
      // Windows exits inside the updater before this promise resolves. macOS
      // and Linux return after replacing the bundle and need this relaunch.
      await restartApplication();
    } catch (caught) {
      setInstalling(false);
      onError(caught as CommandError);
    }
  }, [installing, onError, replaceUpdate, restartRequired]);

  const restartInstalledUpdate = useCallback(async () => {
    if (!restartRequired || installing) return;
    setInstalling(true);
    try {
      await restartApplication();
    } catch (caught) {
      setInstalling(false);
      onError(caught as CommandError);
    }
  }, [installing, onError, restartRequired]);

  return {
    updateCheck,
    checking,
    installing,
    downloadProgress,
    lastCheckedAt,
    restartRequired,
    runUpdateCheck: () => runUpdateCheck(true),
    installAvailableUpdate,
    restartInstalledUpdate,
  };
}
