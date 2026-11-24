import type { ExtensionListItem, McpEditViewEnvelope } from "../../api/client";
import type { SkillUpdatePreparation } from "../../app/extensions/extension-ops";
import { McpEditForm } from "../../components/extensions/McpEditForm";
import { SkillWorkbench } from "../../components/extensions/SkillWorkbench";
import { toast, toastMessage } from "../../components/use-toast";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

export async function deployUpdatedDefinition(w: ExtensionWorkspace, saved: SkillUpdatePreparation | null) {
  if (!saved) return false;
  if (saved.deployment === "unverified") {
    toast({
      kind: "warning",
      title: toastMessage("extensions.editor.savedUnverifiedTitle"),
      description: toastMessage("extensions.editor.redeployHint"),
    });
    return true;
  }
  if (saved.deployment === "notRequired") return true;
  const result = await w.applies.run({
    operations: [{ operation: "update", definitionId: saved.definition.id }],
  });
  if (result.status === "cancelled") toast({
    kind: "info",
    title: toastMessage("extensions.editor.savedCancelledTitle"),
    description: toastMessage("extensions.editor.redeployHint"),
  });
  return true;
}

export function McpEditorDialog({
  workspace: w,
  envelope,
}: {
  workspace: ExtensionWorkspace;
  envelope: McpEditViewEnvelope;
}) {
  return (
      <McpEditForm
        key={envelope.id}
        envelope={envelope}
        currentRevision={w.items.find((item) => item.id === envelope.id)?.revision ?? envelope.revision}
        onReload={() => w.ext.loadMcpEdit(envelope.id)}
        busy={w.writeBlocked}
        onPutSecret={w.ext.putSecret}
        onCancel={w.nav.closeDialog}
        onSave={async (edit) => {
          const result = await w.ext.applyMcpEdit(envelope.id, edit);
          if (!result) return false;
          return deployUpdatedDefinition(w, result);
        }}
      />
  );
}

export function SkillEditorDialog({
  workspace: w,
  item,
}: {
  workspace: ExtensionWorkspace;
  item: Extract<ExtensionListItem, { kind: "skill" }>;
}) {
  return (
      <SkillWorkbench
        item={item}
        mcpOptions={w.items
          .filter((entry) => entry.kind === "mcp")
          .map((entry) => ({ id: entry.id, name: entry.name }))}
        busy={w.writeBlocked}
        onLoadEditor={w.ext.loadSkillEditor}
        onLoadVersions={w.ext.loadSkillVersions}
        onSaveFiles={async (id, update) => {
          const result = await w.ext.saveSkillFiles(id, update);
          return deployUpdatedDefinition(w, result);
        }}
        onRestoreVersion={async (id, digest) => {
          const result = await w.ext.restoreSkillVersion(id, digest, item.revision);
          return deployUpdatedDefinition(w, result);
        }}
        onSaveDependencies={w.ext.saveSkillDependencies}
        onFork={(id) =>
          void w.ext.forkSkill(id).then((result) => {
            if (result) w.nav.openSkillEditor(result.id);
          })
        }
        onClose={w.nav.closeDialog}
      />
  );
}
