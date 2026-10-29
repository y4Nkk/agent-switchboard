import { useCallback, useEffect, useMemo, useState } from "react";
import { onTrayError, openBackupDir, type AppKind, type CommandError } from "../api/client";
import { useAppSettings } from "./useAppSettings";
import { useApplicationDataRepair } from "./useApplicationDataRepair";
import { useCcImport } from "./useCcImport";
import { useCloudBackup } from "./useCloudBackup";
import { useClientSettings } from "./useClientSettings";
import { useCodexSubagentSettings } from "./useCodexSubagentSettings";
import { useConfigSnapshot } from "./useConfigSnapshot";
import { useDiscovery } from "./useDiscovery";
import { useOperationFrame } from "./useOperationFrame";
import { usePromptDocuments } from "./usePromptDocuments";
import { useProviders } from "./useProviders";
import { useSqlImport } from "./useSqlImport";
import { latestOverall, useSwitchOperations } from "./useSwitchOperations";
import { useProviderSwitchFlow } from "./useProviderSwitchFlow";
import { useUpdateCheck } from "./useUpdateCheck";
import { useWorkspaceNavigation } from "./useWorkspaceNavigation";
import { useSidebarState } from "./useSidebarState";
import { useModelRouteNavigation } from "./useModelRouteNavigation";
import { useCodexProbe } from "./useCodexProbe";

function useTrayEvents(reportError: (error: CommandError) => void) {
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void onTrayError(reportError).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    }).catch((error: unknown) => reportError({ code: "TRAY_EVENT", message: error instanceof Error ? error.message : String(error) }));
    return () => { disposed = true; stop?.(); };
  }, [reportError]);
}

/** Composes domain hooks; each domain owns its state and typed operations. */
export function useSwitchboardModel() {
  const probe = useCodexProbe();
  const [appFilter, setAppFilter] = useState<AppKind>("codex");
  const frame = useOperationFrame();
  const { busy, reportError, clearError, setBusy } = frame;
  useTrayEvents(reportError);
  const operationContext = { busy, onError: reportError, clearError, setBusy };
  const appSettingsState = useAppSettings(operationContext);
  const sidebar = useSidebarState(reportError);
  const navigation = useWorkspaceNavigation(
    appSettingsState.appSettings !== null || appSettingsState.loadError !== null,
    appSettingsState.loadError, reportError,
  );
  const { page, setPage } = navigation;
  const modelRoutes = useModelRouteNavigation(navigation.setPage);
  const snapshot = useConfigSnapshot({ onError: reportError });
  const { targetProfileId, setTargetProfileId, refresh: refreshSnapshot, activeProfileId, records } = snapshot;
  const refresh = useCallback(async () => { await refreshSnapshot(); }, [refreshSnapshot]);
  const providerSwitch = useProviderSwitchFlow({
    busy,
    onError: reportError,
    clearError,
    onTargetProfileChange: setTargetProfileId,
  });
  const { invalidateCandidates, setTargetProfile } = providerSwitch;
  const clientSettings = useClientSettings({
    app: appFilter,
    busy,
    active: page === "clientConfiguration",
  });
  const codexSubagentSettings = useCodexSubagentSettings({
    active: page === "clientConfiguration" && appFilter === "codex",
    busy,
    onError: reportError,
  });
  const promptDocuments = usePromptDocuments({
    ...operationContext, active: page === "clientConfiguration",
  });
  const cloudBackup = useCloudBackup({ ...operationContext, invalidateCandidates, refresh });
  const updateCheck = useUpdateCheck({ onError: reportError });
  const discoveryState = useDiscovery({
    ...operationContext, app: appFilter, invalidateCandidates, refresh: refreshSnapshot, setTargetProfile, setAppFilter, setPage,
  });
  const ccImport = useCcImport({ ...operationContext, invalidateCandidates, refresh: refreshSnapshot,
    records, codexRecords: snapshot.codexRecords, preferredApp: appFilter, setTargetProfile, setAppFilter });
  const sqlImport = useSqlImport({ ...operationContext, invalidateCandidates, refresh: refreshSnapshot,
    records, codexRecords: snapshot.codexRecords, preferredApp: appFilter, setTargetProfile, setAppFilter });
  const targetRecord = records.find((record) => record.profile.id === targetProfileId) ?? null;
  const targetProfile = targetRecord?.profile ?? null;
  const operations = useSwitchOperations({
    ...operationContext, activationCandidate: providerSwitch.activationCandidate, clearCandidates: providerSwitch.clearCandidates,
    invalidateCandidates, setTargetProfile, targetProfileId, targetProfile, refresh,
    refreshDiscoveryOrAppend: discoveryState.refreshDiscoveryOrAppend,
  });
  const providers = useProviders({
    ...operationContext, active: page === "providers", appFilter, setAppFilter, records,
    codexOfficialRecords: snapshot.codexOfficialRecords, targetProfileId,
    invalidateCandidates, clearCandidates: providerSwitch.clearCandidates, refresh, setTargetProfile,
    setTargetProfileId,
  });
  const dataRepair = useApplicationDataRepair({ ...operationContext, invalidateCandidates, refresh,
    onProvidersRepaired: providers.closeEditor });
  const lastSwitchOverall = useMemo(() => latestOverall(snapshot.statuses ?? []), [snapshot.statuses]);
  const openBackupFolder = useCallback(
    () => openBackupDir().catch((caught) => reportError(caught as CommandError)), [reportError],
  );

  return { ...navigation, sidebar, appFilter, ...frame, snapshot, activeProfileId, providerSwitch, modelRoutes,
    clientSettings, codexSubagentSettings, promptDocuments, appSettingsState, cloudBackup, updateCheck, discoveryState,
    ccImport, sqlImport, targetProfile, operations, providers, dataRepair, lastSwitchOverall, openBackupFolder, probe };
}

export type SwitchboardModel = ReturnType<typeof useSwitchboardModel>;
