import { useState } from "react";
import type { ExtensionListItem } from "../../api/client";
import { ExtensionManagement } from "../../components/extensions/ExtensionManagement";
import { AppDialog } from "../../components/AppDialog";
import { useI18n } from "../../i18n";
import { parseTargetValue } from "../../components/extensions/labels";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

export function ExtensionManagementDialog({
  workspace: w,
  item,
}: {
  workspace: ExtensionWorkspace;
  item: ExtensionListItem;
}) {
  const { t } = useI18n();
  const [targets, setTargets] = useState<string[]>([]);
  const updateReport = w.updates.reportMap.get(item.id) ?? null;
  const registryUpdate = w.registryUpdates.reportMap.get(item.id);
  return (
    <AppDialog title={t("extensions.management.dialogTitle", { name: item.name })} busy={w.busy} onClose={w.nav.closeDialog} wide>
      <ExtensionManagement
        item={item}
        projects={w.ext.workspace?.projects ?? []}
        capabilities={w.ext.workspace?.capabilities ?? []}
        busy={w.writeBlocked}
        check={w.check}
        installTargets={targets}
        onInstallTargetsChange={setTargets}
        updateReport={updateReport}
        registryUpdate={registryUpdate}
        projectNames={w.projectNames}
        onInstall={() =>
          void w.applies.run({
            operations: [
              {
                operation: "install",
                definitionId: item.id,
                targets: targets.map(parseTargetValue).filter((target) => target !== null),
              },
            ],
          })
        }
        onChangeBinding={(binding, enable) =>
          void w.applies.run({
            operations: [
              {
                operation: enable ? "enable" : "disable",
                bindingId: binding.id,
              },
            ],
          })
        }
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
