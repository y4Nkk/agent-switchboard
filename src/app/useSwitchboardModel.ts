import { useCallback, useMemo, useState } from "react";
import { openBackupDir, type AppKind, type CommandError } from "../api/client";
import type { Page } from "./AppShell";
import { useAppSettings } from "./useAppSettings";
import { useCcImport } from "./useCcImport";
import { useCloudBackup } from "./useCloudBackup";
import { useCommonSettings } from "./useCommonSettings";
import { useConfigSnapshot } from "./useConfigSnapshot";
import { useDiscovery } from "./useDiscovery";
import { useOperationFrame } from "./useOperationFrame";
import { usePromptDocuments } from "./usePromptDocuments";
import { useProviders } from "./useProviders";
import { latestOverall, useSwitchOperations } from "./useSwitchOperations";
import { useSwitchPreview } from "./useSwitchPreview";
import { useUpdateCheck } from "./useUpdateCheck";

/**
 * Root model composition: wires the domain hooks together. Contains no
 * logic of its own — each domain (snapshot, previews, common settings,
 * provider store, switch operations, discovery, CC import, app settings)
 * owns its state and handlers in its own hook.
 */
export function useSwitchboardModel() {
  const [page, setPage] = useState<Page>("概览");
  const [appFilter, setAppFilter] = useState<AppKind>("codex");
  const frame = useOperationFrame();
  const { busy, reportError, clearError } = frame;

  const snapshot = useConfigSnapshot({ onError: reportError });
  const { selectedId, setSelectedId, refresh, activeProfileId, records } = snapshot;
  const switchPreview = useSwitchPreview({
    busy,
    selectedId,
    setSelectedId,
    onError: reportError,
    clearError,
  });
  const { invalidateSwitchCandidates, selectProfile } = switchPreview;
  const commonSettings = useCommonSettings({
    active: page === "通用设置",
    invalidateSwitchCandidates,
    refresh,
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
  });
  const promptDocuments = usePromptDocuments({
    active: page === "通用设置",
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
  });
  /** Supplier drafts and app-store changes invalidate supplier candidates.
   * Common-base drafts never carry client-file candidates; their separate
   * read-only fragment preview does not participate in switching. */
  const invalidateCandidates = useCallback(
    () => invalidateSwitchCandidates(),
    [invalidateSwitchCandidates],
  );

  const appSettingsState = useAppSettings({
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
  });
  const cloudBackup = useCloudBackup({
    busy,
    setBusy: frame.setBusy,
    onError: reportError,
    clearError,
    invalidateCandidates,
    refresh,
  });
  const updateCheck = useUpdateCheck({
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
  });
  const discoveryState = useDiscovery({
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
    invalidateCandidates,
    refresh,
    selectProfile,
    setAppFilter,
    setPage,
  });
  const ccImport = useCcImport({
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
    invalidateCandidates,
    refresh,
  });
  const selectedRecord = records.find((record) => record.profile.id === selectedId) ?? null;
  const selectedProfile = selectedRecord?.profile ?? null;
  const operations = useSwitchOperations({
    busy,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
    preview: switchPreview.preview,
    retractPreview: switchPreview.retractPreview,
    invalidateCandidates,
    selectProfile,
    selectedId,
    selectedProfile,
    refresh,
    refreshDiscoveryOrAppend: discoveryState.refreshDiscoveryOrAppend,
  });
  const providers = useProviders({
    busy,
    appFilter,
    setAppFilter,
    selectedRecord,
    records,
    selectedId,
    onError: reportError,
    clearError,
    setBusy: frame.setBusy,
    invalidateCandidates,
    retractPreview: switchPreview.retractPreview,
    refresh,
    selectProfile,
    setRecords: snapshot.setRecords,
    setSelectedId,
  });

  const lastSwitchOverall = useMemo(
    () => latestOverall(snapshot.statuses ?? []),
    [snapshot.statuses],
  );
  const openBackupFolder = useCallback(
    () => openBackupDir().catch((caught) => reportError(caught as CommandError)),
    [reportError],
  );

  return {
    page,
    setPage,
    appFilter,
    ...frame,
    snapshot,
    activeProfileId,
    switchPreview,
    commonSettings,
    promptDocuments,
    appSettingsState,
    cloudBackup,
    updateCheck,
    discoveryState,
    ccImport,
    selectedProfile,
    operations,
    providers,
    lastSwitchOverall,
    openBackupFolder,
  };
}
