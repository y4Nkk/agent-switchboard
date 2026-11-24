import { useCallback } from "react";
import {
  exportExtensionPortable,
  importExtensionPortable,
  importSkillCandidate,
  resolveSkillSource,
  scanLocalSkillSource,
  type AppKind,
  type ExtensionMutation,
  type PortableImportReport,
  type SkillCandidateDto,
} from "../../api/client";
import { toast, toastMessage } from "../../components/use-toast";
import { tr } from "../../i18n/current";
import type { ExclusiveRunner, WorkspaceRefresher } from "./extension-ops";

interface ImportDeps {
  refresh: WorkspaceRefresher;
  runExclusive: ExclusiveRunner;
}

/** Source scanning, imports, and portable packages. Discovery-scan reads,
 * imports from discovered rows, and takeovers live in useDiscoverScan. */
export function useExtensionImport({ refresh, runExclusive }: ImportDeps) {
  const scanLocal = useCallback(
    (root: string) => runExclusive((): Promise<SkillCandidateDto[]> => scanLocalSkillSource(root)),
    [runExclusive],
  );

  const resolveSource = useCallback(
    (repo: string, subpath: string, refName: string | null) =>
      runExclusive((): Promise<SkillCandidateDto[]> => resolveSkillSource(repo, subpath, refName)),
    [runExclusive],
  );

  const importCandidate = useCallback(
    (candidateId: string, name: string, hostScoped: AppKind | null) =>
      runExclusive(async (): Promise<ExtensionMutation> => {
        const definition = await importSkillCandidate(candidateId, name, hostScoped);
        await refresh();
        toast({ kind: "success", title: toastMessage("extensions.importOp.importedSkill"), description: definition.name });
        return definition;
      }),
    [refresh, runExclusive],
  );

  const exportPortable = useCallback(
    (definitionId: string, targetPath: string) =>
      runExclusive(async (): Promise<boolean> => {
        await exportExtensionPortable(definitionId, targetPath);
        toast({ kind: "success", title: toastMessage("extensions.importOp.exported"), description: targetPath });
        return true;
      }),
    [runExclusive],
  );

  const importPortable = useCallback(
    (packagePath: string) =>
      runExclusive(async (): Promise<PortableImportReport> => {
        const report = await importExtensionPortable(packagePath);
        await refresh();
        const missing = report.missingEnvSlots;
        toast({
          kind: missing.length > 0 ? "warning" : "success",
          title: toastMessage("extensions.importOp.imported"),
          description:
            missing.length > 0
              ? toastMessage("extensions.importOp.missingEnv", { slots: missing.join(tr("extensions.join.comma")) })
              : report.warnings[0] ?? toastMessage("extensions.importOp.added"),
        });
        return report;
      }),
    [refresh, runExclusive],
  );

  return {
    scanLocal,
    resolveSource,
    importCandidate,
    exportPortable,
    importPortable,
  };
}
