import { useCallback, useEffect, useRef, useState } from "react";
import {
  getConfigStatus,
  getLockStatus,
  listBackups,
  listProfiles,
  type AppKind,
  type BackupRecord,
  type CommandError,
  type ConfigFileStatus,
  type LockStatus,
  type ProviderProfile,
} from "../api/client";

interface SnapshotDeps {
  onError: (error: CommandError) => void;
}

/**
 * The observable client snapshot: file statuses, profiles, backups, locks,
 * and the selected profile id. One versioned `refresh` keeps late responses
 * from overwriting newer ones.
 */
export function useConfigSnapshot({ onError }: SnapshotDeps) {
  const [statuses, setStatuses] = useState<ConfigFileStatus[] | null>(null);
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [locks, setLocks] = useState<Partial<Record<AppKind, LockStatus>>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const refreshVersion = useRef(0);

  const refresh = useCallback(async () => {
    const version = ++refreshVersion.current;
    try {
      const [nextStatuses, nextProfiles, nextBackups, codexLock, claudeLock] = await Promise.all([
        getConfigStatus(),
        listProfiles(),
        listBackups(),
        getLockStatus("codex"),
        getLockStatus("claude"),
      ]);
      if (refreshVersion.current !== version) return;
      setStatuses(nextStatuses);
      setProfiles(nextProfiles);
      setBackups(nextBackups);
      setLocks({ codex: codexLock, claude: claudeLock });
      setSelectedId((current) =>
        current && !nextProfiles.some((profile) => profile.id === current) ? null : current,
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

  /** Profile id the live file of one client actually matches, when the app
   * can tell; null otherwise. */
  const activeProfileId = useCallback(
    (app: AppKind) => {
      const status = (statuses ?? []).find((item) => item.app === app);
      if (status?.matchStatus.kind === "matchesProfile") {
        return status.matchStatus.profileId;
      }
      return null;
    },
    [statuses],
  );

  return {
    statuses,
    profiles,
    setProfiles,
    backups,
    locks,
    selectedId,
    setSelectedId,
    refresh,
    activeProfileId,
  };
}
