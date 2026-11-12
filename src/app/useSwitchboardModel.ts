import { useCallback, useEffect, useState } from "react";
import { onTrayError, openBackupDir, type AppKind, type CommandError } from "../api/client";
import { useConfigurationModel } from "./useConfigurationModel";
import { useAppSettings } from "./useAppSettings";
import type { ApplicationPreparation } from "../api/application-preparation";
import { useCloudBackup } from "./useCloudBackup";
import { useClientSettings } from "./useClientSettings";
import { useCodexSubagentSettings } from "./useCodexSubagentSettings";
import { useOperationFrame } from "./useOperationFrame";
import { usePromptDocuments } from "./usePromptDocuments";
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
export function useSwitchboardModel(preparation: ApplicationPreparation, dataRevision: number) {
  const probe = useCodexProbe();
  const [appFilter, setAppFilter] = useState<AppKind>("codex");
  const frame = useOperationFrame();
  const { reportError, clearError, setBusy } = frame;
  const busy = frame.busy || preparation.status === "running" || preparation.status === "blocked";
  useTrayEvents(reportError);
  const operationContext = { busy, onError: reportError, clearError, setBusy };
  useEffect(clearError, [dataRevision, clearError]);
  const appSettingsState = useAppSettings(operationContext, dataRevision);
  const sidebar = useSidebarState(reportError);
  const navigation = useWorkspaceNavigation(
    appSettingsState.appSettings !== null || appSettingsState.loadError !== null,
    appSettingsState.loadError, reportError,
  );
  const { page, setPage } = navigation;
  const modelRoutes = useModelRouteNavigation(navigation.setPage);
  const configuration = useConfigurationModel({
    operationContext: { ...operationContext, busy: busy || !preparation.configurationReady },
    appFilter, setAppFilter, page, setPage, preparation, dataRevision,
  });
  const { providerSwitch, refresh } = configuration;
  const { invalidateCandidates } = providerSwitch;
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
  const openBackupFolder = useCallback(
    () => openBackupDir().catch((caught) => reportError(caught as CommandError)), [reportError],
  );

  return { ...navigation, sidebar, appFilter, ...frame, busy, preparation, dataRevision, ...configuration, modelRoutes,
    clientSettings, codexSubagentSettings, promptDocuments, appSettingsState, cloudBackup, updateCheck, openBackupFolder, probe };
}

export type SwitchboardModel = ReturnType<typeof useSwitchboardModel>;
