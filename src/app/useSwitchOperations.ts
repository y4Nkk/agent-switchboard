import type { MessageKey } from "../i18n";
import { uiMessage } from "../i18n/errors";
import { useCallback, useRef, useState } from "react";
import {
  backupDiff,
  executeSwitch,
  recoverStaleLock,
  restoreBackup,
  undoLastSwitch,
  type AppKind,
  type CommandError,
  type ConfigFileStatus,
  type KeyChange,
  type ProviderProfile,
  type ConfigWriteRecord, type LocalizedMessage } from "../api/client";
import { notifyWriteOutcome } from "./notifications";
import { toastMessage } from "../components/use-toast";
import type { ActivationCandidate } from "./useProviderSwitchFlow";

interface SwitchOperationDeps {
  busy: boolean;
  restoreBusy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  activationCandidate: ActivationCandidate | null;
  clearCandidates: () => void;
  invalidateCandidates: () => void;
  setTargetProfile: (profileId: string) => Promise<void> | void;
  targetProfileId: string | null;
  targetProfile: ProviderProfile | null;
  refresh: () => Promise<void>;
  refreshDiscoveryOrAppend: (warnings: readonly LocalizedMessage[], failureNote: MessageKey) => Promise<LocalizedMessage[]>;
}

/**
 * The executor-transaction operations triggered from the UI: confirmed
 * switch, backup restore, undo, and stale-lock recovery. Every write
 * invalidates all candidates first and refreshes the snapshot after.
 */
export function useSwitchOperations({
  busy,
  restoreBusy,
  onError,
  clearError,
  setBusy,
  activationCandidate,
  clearCandidates,
  invalidateCandidates,
  setTargetProfile,
  targetProfileId,
  targetProfile,
  refresh,
  refreshDiscoveryOrAppend,
}: SwitchOperationDeps) {
  const [undoPending, setUndoPending] = useState<ConfigWriteRecord | null>(null);
  const [undoDiff, setUndoDiff] = useState<
    | { state: "idle" | "loading" }
    | { state: "ready"; changes: KeyChange[] }
    | { state: "error"; error: CommandError }
  >({ state: "idle" });
  const [recoverLockPending, setRecoverLockPending] = useState<AppKind | null>(null);
  const undoDiffVersion = useRef(0);

  const requestUndo = useCallback(
    (target: ConfigWriteRecord) => {
      if (busy) return;
      const version = undoDiffVersion.current + 1;
      undoDiffVersion.current = version;
      setUndoPending(target);
      setUndoDiff({ state: "loading" });
      void backupDiff(target.backupId).then(
        (changes) => {
          if (undoDiffVersion.current === version) setUndoDiff({ state: "ready", changes });
        },
        (caught: CommandError) => {
          if (undoDiffVersion.current === version) {
            setUndoDiff({ state: "error", error: caught });
          }
        },
      );
    },
    [busy],
  );

  const cancelUndo = useCallback(() => {
    undoDiffVersion.current += 1;
    setUndoPending(null);
    setUndoDiff({ state: "idle" });
  }, []);

  const runSwitch = useCallback(async () => {
    if (busy || !activationCandidate || !targetProfile || activationCandidate.profileId !== targetProfileId) return;
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      const result = await executeSwitch(
        activationCandidate.profileId,
        activationCandidate.file.contentHash,
        activationCandidate.file.renderedHash,
        true,
        activationCandidate.file,
      );
      await refresh();
      await setTargetProfile(targetProfileId);
      const warnings = await refreshDiscoveryOrAppend(
        result.warnings,
        "operations.notify.discoveryStale.written",
      );
      notifyWriteOutcome(toastMessage("operations.notify.switched", { name: targetProfile.name }), targetProfile.app, warnings);
    } catch (caught) {
      const commandError = caught as CommandError;
      onError(commandError);
      if (commandError.code === "external-change" || commandError.code === "preview-stale") {
        clearCandidates();
      }
      await refresh();
    } finally {
      setBusy(false);
    }
  }, [
    busy,
    clearError,
    invalidateCandidates,
    onError,
    activationCandidate,
    refresh,
    refreshDiscoveryOrAppend,
    clearCandidates,
    setTargetProfile,
    targetProfileId,
    targetProfile,
    setBusy,
  ]);

  const runRestore = useCallback(
    async (backupId: string, transactionOnly: boolean) => {
      if (restoreBusy) throw uiMessage("backup.history.restoreBusy");
      invalidateCandidates();
      setBusy(true);
      clearError();
      try {
        const result = await restoreBackup(backupId, true, transactionOnly);
        await refresh();
        if (targetProfileId) await setTargetProfile(targetProfileId);
        const warnings = await refreshDiscoveryOrAppend(
          result.warnings,
          "operations.notify.discoveryStale.restored",
        );
        notifyWriteOutcome(toastMessage("operations.notify.restored"), result.preRestoreBackup.app, warnings);
      } finally {
        setBusy(false);
      }
    },
    [
      restoreBusy,
      clearError,
      invalidateCandidates,
      refresh,
      refreshDiscoveryOrAppend,
      setTargetProfile,
      targetProfileId,
      setBusy,
    ],
  );

  const runUndo = useCallback(async () => {
    if (busy || !undoPending || undoDiff.state !== "ready") return;
    const target = undoPending;
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      const result = await undoLastSwitch(target.app, true);
      await refresh();
      if (targetProfileId) await setTargetProfile(targetProfileId);
      const warnings = await refreshDiscoveryOrAppend(
        result.warnings,
        "operations.notify.discoveryStale.undone",
      );
      notifyWriteOutcome(toastMessage("operations.notify.undone"), target.app, warnings);
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      cancelUndo();
      setBusy(false);
    }
  }, [
    busy,
    cancelUndo,
    clearError,
    invalidateCandidates,
    onError,
    refresh,
    refreshDiscoveryOrAppend,
    targetProfileId,
    setTargetProfile,
    setBusy,
    undoDiff.state,
    undoPending,
  ]);

  const runRecoverStaleLock = useCallback(async () => {
    if (busy || !recoverLockPending) return;
    const target = recoverLockPending;
    setBusy(true);
    clearError();
    try {
      await recoverStaleLock(target);
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setRecoverLockPending(null);
      setBusy(false);
    }
  }, [busy, clearError, onError, recoverLockPending, refresh, setBusy]);

  return {
    undoPending,
    undoDiff,
    requestUndo,
    cancelUndo,
    recoverLockPending,
    setRecoverLockPending,
    runSwitch,
    runRestore,
    runUndo,
    runRecoverStaleLock,
  };
}

/** Latest switch log entry across both clients, for the undo affordance. */
export function latestOverall(statuses: ConfigFileStatus[]): ConfigWriteRecord | null {
  const entries = statuses
    .map((status) => status.lastSwitch)
    .filter((entry): entry is ConfigWriteRecord => entry !== null);
  if (entries.length === 0) return null;
  return entries.reduce((latest, entry) => (entry.at > latest.at ? entry : latest));
}
