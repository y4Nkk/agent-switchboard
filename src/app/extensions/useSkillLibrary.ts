import { useCallback } from "react";
import {
  checkSkillUpdates,
  createLocalSkill,
  forkLocalSkill,
  getSkillEditor,
  listSkillVersions,
  setSkillVersion,
  updateSkillDependencies,
  updateSkillFiles,
  type ExtensionMutation,
  type LocalSkillDraft,
  type SkillDependenciesUpdate,
  type SkillEditorView,
  type SkillFilesUpdate,
  type SkillUpdateReport,
  type SkillVersion,
} from "../../api/client";
import { toast, toastMessage } from "../../components/use-toast";
import type {
  ExclusiveRunner,
  ExtensionReader,
  SkillBatchAdvance,
  SkillUpdatePreparation,
  WorkspaceRefresher,
} from "./extension-ops";
import { definitionUpdatePreparation, refreshLibraryWrite } from "./extension-ops";

interface SkillLibraryDeps {
  refresh: WorkspaceRefresher;
  runExclusive: ExclusiveRunner;
  runRead: ExtensionReader;
}

function useSkillSourceUpdates({ refresh, runExclusive }: SkillLibraryDeps) {
  const checkUpdates = useCallback(
    (definitionIds: string[]) =>
      runExclusive((): Promise<SkillUpdateReport[]> => checkSkillUpdates(definitionIds)),
    [runExclusive],
  );

  /** Advances many skill definitions to their freshly checked versions in
   * one exclusive run. Per-item failures are reported, not thrown, so one
   * expired candidate never blocks the rest of the batch; the caller deploys
   * the advanced ids through the shared apply pipeline. */
  const advanceSkillUpdates = useCallback(
    (entries: Array<{ definitionId: string; candidateId: string; expectedRevision: number }>) =>
      runExclusive(async (): Promise<SkillBatchAdvance> => {
        const failed: Array<{ definitionId: string; error: unknown }> = [];
        for (const entry of entries) {
          try {
            await setSkillVersion(entry.definitionId, entry.expectedRevision, { kind: "upstream", candidateId: entry.candidateId });
          } catch (caught) {
            failed.push({
              definitionId: entry.definitionId,
              error: caught,
            });
          }
        }
        const workspace = await refresh();
        if (workspace === null) toast({
          kind: "warning",
          title: toastMessage("extensions.skillOp.batchUnverified"),
          description: toastMessage("extensions.skillOp.batchUnverifiedBody"),
        });
        return { failed, workspace };
      }),
    [refresh, runExclusive],
  );

  const restoreSkillVersion = useCallback(
    (definitionId: string, digest: string, expectedRevision: number) =>
      runExclusive(async (): Promise<SkillUpdatePreparation> => {
        const definition = await setSkillVersion(definitionId, expectedRevision, { kind: "restore", digest });
        return definitionUpdatePreparation(definition, await refresh());
      }),
    [refresh, runExclusive],
  );
  return { checkUpdates, advanceSkillUpdates, restoreSkillVersion };
}

function useLocalSkills({ refresh, runExclusive }: SkillLibraryDeps) {
  const createSkill = useCallback(
    (draft: LocalSkillDraft) =>
      runExclusive(async (): Promise<ExtensionMutation> => {
        const definition = await createLocalSkill(draft);
        await refreshLibraryWrite(refresh, {
          title: toastMessage("extensions.skillOp.created"),
          description: toastMessage("extensions.skillOp.createdBody"),
        });
        return definition;
      }),
    [refresh, runExclusive],
  );

  const forkSkill = useCallback(
    (definitionId: string) =>
      runExclusive(async (): Promise<ExtensionMutation> => {
        const definition = await forkLocalSkill(definitionId);
        await refreshLibraryWrite(refresh, {
          title: toastMessage("extensions.skillOp.forked"),
          description: toastMessage("extensions.skillOp.forkedBody"),
        });
        return definition;
      }),
    [refresh, runExclusive],
  );
  return { createSkill, forkSkill };
}

function useSkillContent({ refresh, runExclusive, runRead }: SkillLibraryDeps) {
  const loadSkillEditor = useCallback(
    (definitionId: string) =>
      runRead((): Promise<SkillEditorView> => getSkillEditor(definitionId)),
    [runRead],
  );

  /** Publishes one edited content version. Whenever enabled bindings exist,
   * the caller immediately deploys the update through the shared apply
   * pipeline. */
  const saveSkillFiles = useCallback(
    (definitionId: string, update: SkillFilesUpdate): Promise<SkillUpdatePreparation | null> =>
      runExclusive(async () => {
        const definition = await updateSkillFiles(definitionId, update);
        return definitionUpdatePreparation(definition, await refresh());
      }),
    [refresh, runExclusive],
  );

  const loadSkillVersions = useCallback(
    (definitionId: string) =>
      runRead((): Promise<SkillVersion[]> => listSkillVersions(definitionId)),
    [runRead],
  );

  const saveSkillDependencies = useCallback(
    (definitionId: string, update: SkillDependenciesUpdate) =>
      runExclusive(async (): Promise<ExtensionMutation> => {
        const definition = await updateSkillDependencies(definitionId, update);
        await refreshLibraryWrite(refresh, {
          title: toastMessage("extensions.skillOp.depsSaved"),
          description: toastMessage("extensions.skillOp.depsSavedBody"),
        });
        return definition;
      }),
    [refresh, runExclusive],
  );

  return {
    loadSkillEditor,
    saveSkillFiles,
    loadSkillVersions,
    saveSkillDependencies,
  };
}

export function useSkillLibrary(deps: SkillLibraryDeps) {
  const sources = useSkillSourceUpdates(deps);
  const local = useLocalSkills(deps);
  const content = useSkillContent(deps);
  return { ...sources, ...local, ...content };
}
