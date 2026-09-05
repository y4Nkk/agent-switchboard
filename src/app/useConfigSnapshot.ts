import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getConfigStatus,
  getLockStatus,
  listBackups,
  listProfiles,
  onTrayChanged,
  type AppKind,
  type BackupRecord,
  type CommandError,
  type ConfigFileStatus,
  type LockStatus,
  type ProviderRecord,
} from "../api/client";

interface SnapshotDeps {
  onError: (error: CommandError) => void;
}

/**
 * The observable client snapshot: file statuses, provider files, backups, and
 * locks, plus the selected profile id. One versioned `refresh` keeps late
 * responses from overwriting newer ones.
 */
export function useConfigSnapshot({ onError }: SnapshotDeps) {
  const [statuses, setStatuses] = useState<ConfigFileStatus[] | null>(null);
  const [records, setRecords] = useState<ProviderRecord[]>([]);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [locks, setLocks] = useState<Partial<Record<AppKind, LockStatus>>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const refreshVersion = useRef(0);

  const profiles = useMemo(() => records.map((record) => record.profile), [records]);

  const refresh = useCallback(async () => {
    const version = ++refreshVersion.current;
    try {
      const [nextStatuses, nextRecords, nextBackups, codexLock, claudeLock] =
        await Promise.all([
          getConfigStatus(),
          listProfiles(),
          listBackups(),
          getLockStatus("codex"),
          getLockStatus("claude"),
        ]);
      if (refreshVersion.current !== version) return;
      setStatuses(nextStatuses);
      setRecords(nextRecords);
      setBackups(nextBackups);
      setLocks({ codex: codexLock, claude: claudeLock });
      setSelectedId((current) =>
        current && nextRecords.some((record) => record.profile.id === current)
          ? current
          : null,
      );
    } catch (caught) {
      if (refreshVersion.current !== version) return;
      onError(caught as CommandError);
    }
  }, [onError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onTrayChanged(() => {
      void refresh();
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((caught) => {
        if (!disposed) onError(caught as CommandError);
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onError, refresh]);

  /** Live provider identity is independent of full configuration equality. */
  const activeProfileId = useCallback(
    (app: AppKind) => {
      const status = (statuses ?? []).find((item) => item.app === app);
      return status?.activeProfileId ?? null;
    },
    [statuses],
  );

  return {
    statuses,
    /** Provider files with their storage revisions; the write boundary. */
    records,
    setRecords,
    /** Display projections of the stored provider files. */
    profiles,
    backups,
    locks,
    selectedId,
    setSelectedId,
    refresh,
    activeProfileId,
  };
}
