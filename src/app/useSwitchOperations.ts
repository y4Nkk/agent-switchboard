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
  type FilePreview,
  type KeyChange,
  type ProviderProfile,
  type ConfigWriteRecord,
} from "../api/client";
import { notifyWriteOutcome } from "./notifications";

interface SwitchOperationDeps {
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  preview: { profileId: string; file: FilePreview } | null;
  retractPreview: () => void;
  invalidateCandidates: () => void;
  selectProfile: (profileId: string) => Promise<void> | void;
  selectedId: string | null;
  selectedProfile: ProviderProfile | null;
  refresh: () => Promise<void>;
  refreshDiscoveryOrAppend: (warnings: string[], failureNote: string) => Promise<string[]>;
}

/**
 * The executor-transaction operations triggered from the UI: confirmed
 * switch, backup restore, undo, and stale-lock recovery. Every write
 * invalidates all candidates first and refreshes the snapshot after.
 */
export function useSwitchOperations({
  busy,
  onError,
  clearError,
  setBusy,
  preview,
  retractPreview,
  invalidateCandidates,
  selectProfile,
  selectedId,
  selectedProfile,
  refresh,
  refreshDiscoveryOrAppend,
}: SwitchOperationDeps) {
  const [confirmingSwitch, setConfirmingSwitch] = useState(false);
  const [undoPending, setUndoPending] = useState<ConfigWriteRecord | null>(null);
  const [undoDiff, setUndoDiff] = useState<
    | { state: "idle" | "loading" }
    | { state: "ready"; changes: KeyChange[] }
    | { state: "error"; message: string }
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
            setUndoDiff({ state: "error", message: caught.message ?? "无法生成撤回差异" });
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
    if (busy || !selectedId || !preview || !selectedProfile) return;
    setConfirmingSwitch(false);
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      const result = await executeSwitch(
        selectedId,
        preview.file.contentHash,
        preview.file.renderedHash,
        true,
      );
      await refresh();
      await selectProfile(selectedId);
      const warnings = await refreshDiscoveryOrAppend(
        result.warnings,
        "配置已写入，但无法刷新本机配置发现结果。",
      );
      notifyWriteOutcome(`已切换到「${selectedProfile.name}」`, selectedProfile.app, warnings);
    } catch (caught) {
      const commandError = caught as CommandError;
      onError(commandError);
      if (commandError.code === "external-change" || commandError.code === "preview-stale") {
        retractPreview();
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
    preview,
    refresh,
    refreshDiscoveryOrAppend,
    retractPreview,
    selectProfile,
    selectedId,
    selectedProfile,
    setBusy,
  ]);

  const runRestore = useCallback(
    async (backupId: string) => {
      if (busy) return;
      invalidateCandidates();
      setBusy(true);
      clearError();
      try {
        const result = await restoreBackup(backupId, true);
        await refresh();
        if (selectedId) await selectProfile(selectedId);
        const warnings = await refreshDiscoveryOrAppend(
          result.warnings,
          "配置已恢复，但无法刷新本机配置发现结果。",
        );
        notifyWriteOutcome("已恢复备份", result.preRestoreBackup.app, warnings);
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
      refreshDiscoveryOrAppend,
      selectProfile,
      selectedId,
      setBusy,
    ],
  );

  const runUndo = useCallback(async () => {
    if (busy || !undoPending || undoDiff.state !== "ready") return;
    const target = undoPending;
    cancelUndo();
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      const result = await undoLastSwitch(target.app, true);
      await refresh();
      if (selectedId) await selectProfile(selectedId);
      const warnings = await refreshDiscoveryOrAppend(
        result.warnings,
        "配置已撤回，但无法刷新本机配置发现结果。",
      );
      notifyWriteOutcome("已撤回上一次切换", target.app, warnings);
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
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
    selectedId,
    selectProfile,
    setBusy,
    undoDiff.state,
    undoPending,
  ]);

  const runRecoverStaleLock = useCallback(async () => {
    if (busy || !recoverLockPending) return;
    const target = recoverLockPending;
    setRecoverLockPending(null);
    setBusy(true);
    clearError();
    try {
      await recoverStaleLock(target);
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, clearError, onError, recoverLockPending, refresh, setBusy]);

  return {
    confirmingSwitch,
    setConfirmingSwitch,
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
