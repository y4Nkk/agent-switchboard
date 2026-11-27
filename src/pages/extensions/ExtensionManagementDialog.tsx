import { useState } from "react";
import type { BindingStatus, ExtensionListItem } from "../../api/client";
import { ExtensionManagement } from "../../components/extensions/ExtensionManagement";
import { AppDialog } from "../../components/AppDialog";
import { useI18n } from "../../i18n";
import { uiMessage } from "../../i18n/errors";
import { parseTargetValue, targetValue } from "../../components/extensions/labels";
import type { BindingFeedback } from "../../components/extensions/management-types";
import { pendingDeployment } from "./pending-deployment";
import type { ExtensionApplyResult } from "./useExtensionApplies";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

function feedbackForToggle(result: ExtensionApplyResult, binding: BindingStatus): BindingFeedback | null {
  switch (result.status) {
    case "failed": return uiMessage("extensions.apply.rejectedTitle");
    case "rejected": return { kind: "failed", message: result.message };
    case "unverified": return uiMessage("extensions.apply.appliedRefreshFailed");
    case "rolledBack":
    case "applied": {
      const target = targetValue(binding.target);
      const outcome = result.record?.resources.find((record) => record.definitionId === binding.resourceId)
        ?.targets.find((entry) => targetValue(entry.target) === target)?.outcome;
      if (outcome && outcome.kind !== "applied") return outcome;
      return result.status === "rolledBack" ? uiMessage("extensions.apply.failedRolledBack") : null;
    }
    default: return null;
  }
}

export function ExtensionManagementDialog({
  workspace: w,
  item,
}: {
  workspace: ExtensionWorkspace;
  item: ExtensionListItem;
}) {
  const { t } = useI18n();
  const [targets, setTargets] = useState<string[]>([]);
  const [bindingFeedback, setBindingFeedback] = useState(() => new Map<string, BindingFeedback>());
  const updateReport = w.updates.reportMap.get(item.id) ?? null;
  const registryUpdate = w.registryUpdates.reportMap.get(item.id);
  const pendingBindings = new Set(item.bindings.filter((binding) =>
    pendingDeployment(w.applies.pendingOperations, item, binding.target)).map((binding) => binding.id));
  const changeBinding = async (binding: BindingStatus, enable: boolean) => {
    const result = await w.applies.run({
      operations: [{ operation: enable ? "enable" : "disable", bindingId: binding.id }],
    });
    if (result.status === "cancelled" || result.status === "blocked") return;
    const feedback = feedbackForToggle(result, binding);
    setBindingFeedback((current) => {
      if (!feedback && !current.has(binding.id)) return current;
      const next = new Map(current);
      if (feedback) next.set(binding.id, feedback);
      else next.delete(binding.id);
      return next;
    });
  };
  return (
    <AppDialog title={t("extensions.management.dialogTitle", { name: item.name })} busy={w.busy} onClose={w.nav.closeDialog} wide>
      <ExtensionManagement
        item={item}
        projects={w.ext.workspace?.projects ?? []}
        capabilities={w.ext.workspace?.capabilities ?? []}
        busy={w.writeBlocked}
        pendingBindings={pendingBindings}
        bindingFeedback={bindingFeedback}
        check={w.check}
        installTargets={targets}
        onInstallTargetsChange={setTargets}
        updateReport={updateReport}
        registryUpdate={registryUpdate}
        projectNames={w.projectNames}
        onInstall={(selected) =>
          void w.applies.run({
            operations: [
              {
                operation: "install",
                definitionId: item.id,
                targets: selected.map(parseTargetValue).filter((target) => target !== null),
              },
            ],
          }).then((result) => {
            if (result.status === "applied") setTargets((current) => current.filter((value) => !selected.includes(value)));
          })
        }
        onChangeBinding={(binding, enable) => void changeBinding(binding, enable)}
        onRemoveBinding={(binding) =>
          void w.applies.run({ operations: [{ operation: "remove", bindingId: binding.id }] })
        }
        onToggleLock={(binding, locked) => void w.ext.toggleBindingLock(binding.id, locked)}
        onDelete={() => w.nav.setDialog({ type: "remove", item })}
        onCheckUpdates={() => void (item.kind === "skill" ? w.updates.check([item.id]) : w.registryUpdates.check([item.id]))}
        onApplyUpdate={() => {
          if (registryUpdate?.status === "available") w.nav.setDialog({ type: "mcpRegistryUpdate", report: registryUpdate });
          else if (updateReport?.status === "available") void w.updates.update([updateReport]);
        }}
        onEditUpdateConfig={() => void w.edit(item)}
        onDeployCurrent={() =>
          void w.applies.run({ operations: [{ operation: "update", definitionId: item.id }] })
        }
        onExportPortable={
          item.kind === "skill" || item.transport === "stdio"
            ? () => w.nav.setDialog({ type: "export", item })
            : undefined
        }
      />
    </AppDialog>
  );
}
