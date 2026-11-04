import { useEffect } from "react";
import type { AppKind, ExtensionDraft, ExtensionListItem } from "../../api/client";
import type { ExtensionsDeps } from "../../app/extensions/extension-ops";
import {
  clientBindingState,
  clientDeploymentOperations,
  clientDeployState,
  EXTENSION_CLIENTS,
} from "../../app/extensions/deployment-state";
import { useDiscoverScan } from "../../app/extensions/useDiscoverScan";
import { useExtensions } from "../../app/useExtensions";
import { toast, toastMessage } from "../../components/use-toast";
import { searchNeedle } from "./list-filters";
import { useExtensionApplies } from "./useExtensionApplies";
import { useExtensionView, type ExtensionNavigation } from "./useExtensionView";
import { useSkillUpdates } from "./useSkillUpdates";
import { useMcpConnectionCheck } from "./useMcpConnectionCheck";

function definitionActions(
  ext: ReturnType<typeof useExtensions>,
  applies: ReturnType<typeof useExtensionApplies>,
  nav: ReturnType<typeof useExtensionView>,
) {
  const install = (definitionId: string, clients: AppKind[]) => applies.run({
    operations: [{ operation: "install", definitionId,
      targets: clients.map((client) => ({ scope: "app" as const, client })) }],
  });
  const edit = async (item: ExtensionListItem) => {
    if (item.kind === "skill") nav.openSkillEditor(item.id);
    else {
      const envelope = await ext.loadMcpEdit(item.id);
      if (envelope) nav.setDialog({ type: "mcpEditor", envelope });
    }
  };
  const deleteDefinition = async (item: ExtensionListItem): Promise<boolean> => {
    if (item.bindings.length > 0) {
      const result = await applies.run({
        operations: item.bindings.map((binding) => ({ operation: "remove", bindingId: binding.id })),
      });
      if (result.status !== "applied") return false;
    }
    const removed = await ext.removeDefinition(item.id);
    if (removed) nav.closeDialog();
    return removed === true;
  };
  const importCandidate = async (digest: string, name: string, hostScoped: AppKind | null) => {
    const definition = await ext.importCandidate(digest, name, hostScoped);
    if (!definition) return null;
    const result = await install(definition.id, hostScoped ? [hostScoped] : EXTENSION_CLIENTS);
    if (result.status !== "applied") toast({
      kind: "warning", title: toastMessage("extensions.workspace.skillImportPartial"),
      description: toastMessage("extensions.editor.redeployHint"),
    });
    return definition;
  };
  const createMcp = async (draft: ExtensionDraft, clients: AppKind[]): Promise<boolean> => {
    const definition = await ext.saveDefinition(draft);
    if (!definition) return false;
    if (clients.length > 0 && (await install(definition.id, clients)).status !== "applied") {
      toast({
        kind: "warning",
        title: toastMessage("extensions.workspace.mcpSavePartial"),
        description: toastMessage("extensions.editor.redeployHint"),
      });
    }
    nav.closeDialog();
    return true;
  };
  return { edit, deleteDefinition, importCandidate, createMcp };
}

export function useExtensionWorkspace(deps: ExtensionsDeps & ExtensionNavigation & { active: boolean }) {
  const ext = useExtensions(deps);
  const discovery = useDiscoverScan({
    refresh: ext.refresh,
    runExclusive: ext.runExclusive,
    onError: deps.onError,
  });
  const nav = useExtensionView(deps);
  const check = useMcpConnectionCheck(
    nav.dialog?.type === "management" ? nav.dialog.definitionId : null,
    ext.refresh, deps.onError,
  );
  const applies = useExtensionApplies(ext, discovery);
  const updates = useSkillUpdates(ext, applies);
  const items = ext.workspace?.items ?? [];
  const kindItems = items.filter((item) => item.kind === nav.kind);
  const visible = kindItems.filter(
    (item) =>
      searchNeedle(item).toLowerCase().includes(nav.search.trim().toLowerCase()),
  );
  const projectNames = new Map(ext.workspace?.projects.map((project) => [project.id, project.displayName]));
  const resourceNames = new Map(items.map((item) => [item.id, item.name]));
  const bindingInfo = new Map(
    items.flatMap((item) =>
      item.bindings.map(
        (binding) =>
          [binding.id, { name: item.name, kind: item.kind, client: binding.target.client }] as const,
      ),
    ),
  );
  const busy = ext.busy || applies.phase !== null;
  const releaseDiscovery = discovery.release;
  useEffect(() => {
    if (!deps.active && !busy && nav.dialog === null && !discovery.scanning) releaseDiscovery();
  }, [deps.active, busy, nav.dialog, discovery.scanning, releaseDiscovery]);
  const writeBlocked = busy || !ext.workspace || ext.workspace.recoveryRequired.length > 0;
  const toggleClient = (item: ExtensionListItem, client: AppKind) =>
    applies.run({
      operations: clientDeploymentOperations([item], client, !clientBindingState(item, client).all),
    });
  const toggleAll = (client: AppKind) =>
    applies.run({
      operations: clientDeploymentOperations(kindItems, client, !clientDeployState(kindItems, client).all),
    });
  return {
    ext,
    discovery: { ...discovery, repairPreparing: applies.pendingKind === "repair" && applies.phase === "preparing" },
    nav,
    applies,
    updates,
    check,
    items,
    kindItems,
    visible,
    projectNames,
    resourceNames,
    bindingInfo,
    busy,
    writeBlocked,
    toggleClient,
    toggleAll,
    ...definitionActions(ext, applies, nav),
  };
}

export type ExtensionWorkspace = ReturnType<typeof useExtensionWorkspace>;
