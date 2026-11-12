import { useCallback, useEffect, useMemo, useRef, type Dispatch, type SetStateAction } from "react";
import type { AppKind, CommandError } from "../api/client";
import type { ApplicationPreparation } from "../api/application-preparation";
import type { Page } from "./navigation";
import { useConfigSnapshot } from "./useConfigSnapshot";
import { useProviderSwitchFlow } from "./useProviderSwitchFlow";
import { useDiscovery } from "./useDiscovery";
import { useCcImport } from "./useCcImport";
import { useSqlImport } from "./useSqlImport";
import { useProviders } from "./useProviders";
import { latestOverall, useSwitchOperations } from "./useSwitchOperations";

interface Context {
  busy: boolean; onError: (error: CommandError) => void;
  clearError: () => void; setBusy: (busy: boolean) => void;
}

export function useConfigurationModel({ operationContext, appFilter, setAppFilter, page, setPage, preparation, dataRevision }: {
  operationContext: Context; appFilter: AppKind; setAppFilter: Dispatch<SetStateAction<AppKind>>;
  page: Page; setPage: (page: Page) => void; preparation: ApplicationPreparation; dataRevision: number;
}) {
  const { busy, onError: reportError, clearError } = operationContext;
  const snapshot = useConfigSnapshot({ onError: reportError, revision: dataRevision });
  const { targetProfileId, setTargetProfileId, refresh: refreshSnapshot, activeProfileId, records } = snapshot;
  const refresh = useCallback(async () => { await refreshSnapshot(); }, [refreshSnapshot]);
  const providerSwitch = useProviderSwitchFlow({
    busy,
    onError: reportError,
    clearError,
    onTargetProfileChange: setTargetProfileId,
  });
  const { invalidateCandidates, setTargetProfile } = providerSwitch;
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
  const handledPreparation = useRef(-1);
  useEffect(() => {
    if (dataRevision <= handledPreparation.current) return;
    if (preparation.status === "running" || preparation.status === "blocked") return;
    handledPreparation.current = dataRevision;
    if (preparation.repairedFiles.some((path) => path.startsWith("configuration/"))) {
      invalidateCandidates(); providers.closeEditor();
    }
  }, [dataRevision, preparation.status, preparation.repairedFiles, invalidateCandidates, providers.closeEditor]);
  const lastSwitchOverall = useMemo(() => latestOverall(snapshot.statuses ?? []), [snapshot.statuses]);
  return { snapshot, activeProfileId, providerSwitch, discoveryState, ccImport, sqlImport,
    targetProfile, operations, providers, lastSwitchOverall, refresh };
}
