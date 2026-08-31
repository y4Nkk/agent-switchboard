import { useCallback, useState } from "react";
import {
  discoverLocal,
  importDiscoveredProfile,
  type AppKind,
  type CommandError,
  type DiscoveryReport,
} from "../api/client";

interface DiscoveryDeps {
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  invalidateCandidates: () => void;
  refresh: () => Promise<void>;
  selectProfile: (profileId: string) => Promise<void> | void;
  setAppFilter: (app: AppKind) => void;
  setPage: (page: "供应商") => void;
}

/**
 * Local configuration discovery: read-only scanning plus importing a
 * discovered provider into the profile store. Switch operations refresh the
 * discovery result after each write so the 发现 page never goes stale.
 */
export function useDiscovery({
  busy,
  onError,
  clearError,
  setBusy,
  invalidateCandidates,
  refresh,
  selectProfile,
  setAppFilter,
  setPage,
}: DiscoveryDeps) {
  const [discovery, setDiscovery] = useState<DiscoveryReport | null>(null);

  /** Refreshes discovery after a successful write and returns the warnings
   * to report: the incoming ones, plus a note when the refresh itself
   * failed. The write still succeeded, so a failure is a warning, not an
   * error. */
  const refreshDiscoveryOrAppend = useCallback(
    async (warnings: string[], failureNote: string): Promise<string[]> => {
      try {
        setDiscovery(await discoverLocal());
        return warnings;
      } catch {
        return [...warnings, failureNote];
      }
    },
    [],
  );

  const runDiscovery = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    clearError();
    try {
      setDiscovery(await discoverLocal());
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, clearError, onError, setBusy]);

  const runImport = useCallback(
    async (app: AppKind) => {
      if (busy) return;
      invalidateCandidates();
      setBusy(true);
      clearError();
      try {
        const profile = await importDiscoveredProfile(app);
        setAppFilter(profile.app);
        setPage("供应商");
        setDiscovery(null);
        await refresh();
        await selectProfile(profile.id);
      } catch (caught) {
        onError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [
      busy,
      clearError,
      invalidateCandidates,
      onError,
      refresh,
      selectProfile,
      setAppFilter,
      setBusy,
      setPage,
    ],
  );

  return { discovery, runDiscovery, runImport, refreshDiscoveryOrAppend };
}
