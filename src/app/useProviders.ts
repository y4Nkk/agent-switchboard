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
} from "../api/client";

export type EditorMode = "new" | "edit" | null;

interface ProvidersDeps {
  busy: boolean;
  appFilter: AppKind;
  setAppFilter: (app: AppKind) => void;
  selectedProfile: ProviderProfile | null;
  selectedId: string | null;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
  invalidateCandidates: () => void;
  retractPreview: () => void;
  refresh: () => Promise<void>;
  selectProfile: (profileId: string) => Promise<void> | void;
  setProfiles: (profiles: ProviderProfile[]) => void;
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
  selectedProfile,
  selectedId,
  onError,
  clearError,
  setBusy,
  invalidateCandidates,
  retractPreview,
  refresh,
  selectProfile,
  setProfiles,
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

  /** Persists a drag reorder of the visible client's profiles. The store's
   * profiles vector is the single owner of display order; the returned list
   * is the same source of truth after the write. */
  const dragReorderProfiles = useCallback(
    async (orderedIds: string[]) => {
      if (busy) return;
      clearError();
      try {
        setProfiles(await reorderProfiles(appFilter, orderedIds));
      } catch (caught) {
        onError(caught as CommandError);
      }
    },
    [appFilter, busy, clearError, onError, setProfiles],
  );

  const saveProfile = useCallback(
    async (draft: ProviderDraft) => {
      invalidateCandidates();
      setBusy(true);
      clearError();
      try {
        const saved =
          editorMode === "edit" && selectedProfile
            ? await updateProfile(selectedProfile.id, draft)
            : await createProfile(draft);
        setAppFilter(saved.app);
        setEditorMode(null);
        await refresh();
        await selectProfile(saved.id);
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
      selectedProfile,
      setAppFilter,
      setBusy,
    ],
  );

  const runDelete = useCallback(async () => {
    if (busy || !deletePending) return;
    const target = deletePending;
    setDeletePending(null);
    invalidateCandidates();
    setBusy(true);
    clearError();
    try {
      await deleteProfile(target.id);
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
    runDelete,
    runResetStore,
  };
}
