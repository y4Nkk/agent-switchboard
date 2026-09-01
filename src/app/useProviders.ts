import { useCallback, useState } from "react";
import {
  createProfile,
  deleteProfile,
  reorderProfiles,
  resetProfileStore,
  updateProfile,
  type AppKind,
  type CommandError,
  type ProviderDraft,
  type ProviderProfile,
  type ProviderRecord,
  type UsageQuery,
} from "../api/client";

export type EditorMode = "new" | "edit" | null;

interface ProvidersDeps {
  busy: boolean;
  appFilter: AppKind;
  setAppFilter: (app: AppKind) => void;
  /** Storage revision of the provider file being edited; a save refuses to
   * overwrite a file changed outside the application. */
  selectedRecord: ProviderRecord | null;
  records: ProviderRecord[];
  selectedId: string | null;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  invalidateCandidates: () => void;
  retractPreview: () => void;
  refresh: () => Promise<void>;
  selectProfile: (profileId: string) => Promise<void> | void;
  setRecords: (records: ProviderRecord[]) => void;
  setSelectedId: (id: string | null) => void;
}

/**
 * Provider-profile store operations: editor navigation, create, update,
 * delete, reset, and drag reorder. Store writes never touch live client
 * configuration.
 */
export function useProviders({
  busy,
  appFilter,
  setAppFilter,
  selectedRecord,
  records,
  selectedId,
  onError,
  clearError,
  setBusy,
  invalidateCandidates,
  retractPreview,
  refresh,
  selectProfile,
  setRecords,
  setSelectedId,
}: ProvidersDeps) {
  const [editorMode, setEditorMode] = useState<EditorMode>(null);
  const [deletePending, setDeletePending] = useState<ProviderProfile | null>(null);
  const [resetStorePending, setResetStorePending] = useState(false);

  const openEditor = useCallback(
    (profile: ProviderProfile) => {
      retractPreview();
      setSelectedId(profile.id);
      setEditorMode("edit");
    },
    [retractPreview, setSelectedId],
  );

  /** Switching the visible client retracts the preview and clears the
   * selection; nothing is carried across clients. */
  const selectApp = useCallback(
    (app: AppKind) => {
      retractPreview();
      setAppFilter(app);
      setSelectedId(null);
    },
    [retractPreview, setAppFilter, setSelectedId],
  );

  /** Persists a drag reorder of the visible client's provider files. Each
   * file carries its own sort position; the returned list is the same source
   * of truth after the write. */
  const dragReorderProfiles = useCallback(
    async (orderedIds: string[]) => {
      if (busy) return;
      const expectedFileHashes = Object.fromEntries(
        records
          .filter((record) => record.profile.app === appFilter)
          .map((record) => [record.profile.id, record.fileHash]),
      );
      setBusy(true);
      clearError();
      try {
        setRecords(await reorderProfiles(appFilter, orderedIds, expectedFileHashes));
      } catch (caught) {
        onError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [appFilter, busy, clearError, onError, records, setBusy, setRecords],
  );

  const saveProfile = useCallback(
    async (draft: ProviderDraft) => {
      invalidateCandidates();
      setBusy(true);
      clearError();
      try {
        const saved =
          editorMode === "edit" && selectedRecord
            ? await updateProfile(selectedRecord.profile.id, draft, selectedRecord.fileHash)
            : await createProfile(draft);
        setAppFilter(saved.profile.app);
        setEditorMode(null);
        await refresh();
        await selectProfile(saved.profile.id);
      } catch (caught) {
        onError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [
      clearError,
      editorMode,
      invalidateCandidates,
      onError,
      refresh,
      selectProfile,
      selectedRecord,
      setAppFilter,
      setBusy,
    ],
  );

  const saveProfileUsageQuery = useCallback(
    async (profile: ProviderProfile, usageQuery: UsageQuery | null): Promise<boolean> => {
      if (busy) return false;
      const record = records.find((candidate) => candidate.profile.id === profile.id);
      if (!record) return false;
      setBusy(true);
      clearError();
      try {
        const { id, ...draft } = profile;
        await updateProfile(id, { ...draft, usageQuery }, record.fileHash);
        await refresh();
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, records, refresh, setBusy],
  );

  const runDelete = useCallback(async () => {
    if (busy || !deletePending) return;
    const target = deletePending;
    const record = records.find((candidate) => candidate.profile.id === target.id);
    setDeletePending(null);
    if (!record) {
      onError({ code: "profile-not-found", message: "供应商已不存在，请重新读取" });
      return;
    }
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      await deleteProfile(target.id, record.fileHash);
      if (selectedId === target.id) {
        setSelectedId(null);
      }
      setEditorMode(null);
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [
    busy,
    clearError,
    deletePending,
    invalidateCandidates,
    onError,
    records,
    refresh,
    selectedId,
    setBusy,
    setSelectedId,
  ]);

  const runResetStore = useCallback(async () => {
    if (busy || !resetStorePending) return;
    setResetStorePending(false);
    invalidateCandidates();
    setBusy(true);
    clearError();
    setSelectedId(null);
    setEditorMode(null);
    try {
      await resetProfileStore(true);
      await refresh();
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [
    busy,
    clearError,
    invalidateCandidates,
    onError,
    refresh,
    resetStorePending,
    setBusy,
    setSelectedId,
  ]);

  return {
    editorMode,
    setEditorMode,
    deletePending,
    setDeletePending,
    resetStorePending,
    setResetStorePending,
    openEditor,
    selectApp,
    dragReorderProfiles,
    saveProfile,
    saveProfileUsageQuery,
    runDelete,
    runResetStore,
  };
}
