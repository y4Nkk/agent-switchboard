import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getConfigStatus,
  getLockStatus,
  listBackups,
  listCodexProfiles,
  listProfiles,
  onClientConfigChanged,
  onTrayChanged,
  type AppKind,
  type BackupRecord,
  type CommandError,
  type CodexProviderRecord,
  type ConfigFileStatus,
  type LockStatus,
  type ProviderRecord,
} from "../api/client";
import { codexLoginBlocker } from "../api/official-login";
import type { ActiveProfileRef } from "../lib/current-provider-name";

interface SnapshotDeps {
  revision: number;
  onError: (error: CommandError) => void;
}

/** The complete set of provider records produced by one consistent refresh. */
export interface ProviderInventory {
  claude: ProviderRecord[];
  codex: CodexProviderRecord[];
}

/**
 * The observable client snapshot: file statuses, provider files, backups, and
 * locks, plus the target profile id. One versioned `refresh` keeps late
 * responses from overwriting newer ones.
 */
export function useConfigSnapshot({ onError, revision }: SnapshotDeps) {
  const [statuses, setStatuses] = useState<ConfigFileStatus[] | null>(null);
  const [records, setRecords] = useState<ProviderRecord[]>([]);
  const [codexOfficialRecords, setCodexOfficialRecords] = useState<ProviderRecord[]>([]);
  const [codexRecords, setCodexRecords] = useState<CodexProviderRecord[]>([]);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [locks, setLocks] = useState<Partial<Record<AppKind, LockStatus>>>({});
  const [loginBlocker, setLoginBlocker] = useState<string | null>(null);
  const [targetProfileId, setTargetProfileId] = useState<string | null>(null);
  const refreshVersion = useRef(0);

  const profiles = useMemo(() => records.map((record) => record.profile), [records]);

  /** The route cards' lookup source: every stored profile across both
   * clients. The generic store carries websiteUrl on the profile, while
   * Codex third-party files carry it at record level. */
  const relayProfiles = useMemo<ActiveProfileRef[]>(() => [
    ...records.map((record) => record.profile),
    ...codexOfficialRecords.map((record) => record.profile),
    ...codexRecords.map((record) => ({
      ...record.profile,
      app: "codex" as const,
      websiteUrl: record.websiteUrl,
    })),
  ], [records, codexOfficialRecords, codexRecords]);

  const refresh = useCallback(async () => {
    const version = ++refreshVersion.current;
    try {
      const [
        nextStatuses,
        allRecords,
        nextCodexRecords,
        nextBackups,
        codexLock,
        claudeLock,
        nextLoginBlocker,
      ] = await Promise.all([
        getConfigStatus(),
        listProfiles(),
        listCodexProfiles(),
        listBackups(),
        getLockStatus("codex"),
        getLockStatus("claude"),
        codexLoginBlocker(),
      ]);
      if (refreshVersion.current !== version) return;
      setStatuses(nextStatuses);
      // The generic store serves Claude providers and the Codex official-login
      // record; Codex third-party providers come from their own strict store.
      const nextRecords = allRecords.filter((record) => record.profile.app === "claude");
      const nextCodexOfficial = allRecords.filter((record) => record.profile.app === "codex");
      setRecords(nextRecords);
      setCodexOfficialRecords(nextCodexOfficial);
      setCodexRecords(nextCodexRecords);
      setBackups(nextBackups);
      setLocks({ codex: codexLock, claude: claudeLock });
      setLoginBlocker(nextLoginBlocker);
      setTargetProfileId((current) =>
        current && (nextRecords.some((record) => record.profile.id === current)
          || nextCodexOfficial.some((record) => record.profile.id === current)
          || nextCodexRecords.some((record) => record.profile.id === current))
          ? current
          : null,
      );
      return { claude: nextRecords, codex: nextCodexRecords } satisfies ProviderInventory;
    } catch (caught) {
      if (refreshVersion.current !== version) return;
      onError(caught as CommandError);
    }
  }, [onError]);

  useEffect(() => {
    void refresh();
  }, [refresh, revision]);

  useEffect(() => {
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onClientConfigChanged(() => {
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
    /** Claude provider files with their storage revisions; the write boundary. */
    records,
    /** The Codex official-login record, stored in the same generic boundary. */
    codexOfficialRecords,
    codexRecords,
    setCodexRecords,
    /** Display projections of the stored provider files. */
    profiles,
    /** Every stored profile flattened for the route cards' active-profile
     * lookups, across both clients and all three stores. */
    relayProfiles,
    backups,
    locks,
    /** Why third-party Codex switching is currently blocked, or null. */
    loginBlocker,
    targetProfileId,
    setTargetProfileId,
    refresh,
    activeProfileId,
  };
}
