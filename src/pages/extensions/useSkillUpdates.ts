import { createElement, useState } from "react";
import type { ExtensionListItem, SkillUpdate, SkillUpdateReport } from "../../api/client";
import type { useExtensions } from "../../app/useExtensions";
import { CommandErrorLines } from "../../app/notifications";
import { ToastMessageList } from "../../components/ToastContent";
import { toast, toastMessage } from "../../components/use-toast";
import type { ExtensionApplies } from "./useExtensionApplies";

type Extensions = ReturnType<typeof useExtensions>;

function isSaved(item: ExtensionListItem | undefined, report: SkillUpdate) {
  return item?.kind === "skill" && item.contentDigest === report.digest && item.source?.resolvedCommit === report.commit;
}

function canUpdate(report: SkillUpdate, items: ExtensionListItem[]) {
  const item = items.find((entry) => entry.id === report.definitionId);
  return item?.kind === "skill" && (!isSaved(item, report) ? item.revision === report.revision :
    item.bindings.some((binding) => binding.desired === "enabled" &&
      !binding.lockedDigest && binding.fileState === "pendingApply"));
}

async function updateSkills(
  ext: Extensions,
  applies: ExtensionApplies,
  available: SkillUpdate[],
  selected: SkillUpdate[],
  complete: (ids: string[]) => void,
): Promise<boolean> {
  const chosen = [...new Map(selected.filter((report) => available.some((entry) =>
    entry.definitionId === report.definitionId && entry.candidateId === report.candidateId && entry.revision === report.revision))
    .map((report) => [report.definitionId, report])).values()];
  if (chosen.length === 0) return false;
  const items = ext.workspace?.items ?? [];
  const entries = chosen.flatMap((report) => {
    const item = items.find((entry) => entry.id === report.definitionId);
    return item?.kind === "skill" && !isSaved(item, report)
      ? [{ definitionId: report.definitionId, candidateId: report.candidateId, expectedRevision: report.revision }] : [];
  });
  const result = entries.length > 0 ? await ext.advanceSkillUpdates(entries)
    : { failed: [], workspace: ext.workspace };
  if (!result) return false;
  if (result.failed.length > 0) toast({
    kind: "warning", title: toastMessage("extensions.updates.partialFailure"),
    description: createElement(ToastMessageList, { items: result.failed.map((failure) =>
      createElement("div", null,
        items.find((item) => item.id === failure.definitionId)?.name ?? failure.definitionId,
        createElement(CommandErrorLines, { error: failure.error }))) }),
  });
  if (result.workspace === null) return false;
  const currentItems = result.workspace.items;
  const ready = chosen.filter((report) => isSaved(currentItems.find((item) => item.id === report.definitionId), report))
    .map((report) => report.definitionId);
  const operations = ready.filter((id) => currentItems.some((item) => item.id === id &&
    item.bindings.some((binding) => binding.desired === "enabled" && !binding.lockedDigest)))
    .map((definitionId) => ({ operation: "update" as const, definitionId }));
  if (operations.length > 0) {
    const applied = await applies.run({ operations });
    if (applied.status !== "applied") {
      complete(ready.filter((id) => !operations.some((operation) => operation.definitionId === id)));
      return false;
    }
  } else if (ready.length > 0) toast({ kind: "success", title: toastMessage("extensions.updates.updatedToLibrary") });
  complete(ready);
  return ready.length > 0 && result.failed.length === 0;
}

export function useSkillUpdates(ext: Extensions, applies: ExtensionApplies) {
  const [reports, setReports] = useState<SkillUpdateReport[]>([]);
  const items = ext.workspace?.items ?? [];
  const reportMap = new Map(reports.filter((report) => items.some((item) => item.id === report.definitionId &&
    item.revision === report.revision)).map((report) => [report.definitionId, report]));
  const updatable = reports.filter((report): report is SkillUpdate => report.status === "available" && canUpdate(report, items));
  const check = async (ids: string[]) => {
    if (ids.length === 0) return;
    const result = await ext.checkUpdates(ids);
    if (!result) return;
    setReports((previous) => [...result, ...previous.filter((report) => !ids.includes(report.definitionId))]);
    const failures = result.filter((report) => report.status === "failed");
    if (failures.length > 0) toast({
      kind: "warning", title: toastMessage("extensions.updates.checkFailedCount", { count: failures.length }),
      description: createElement(ToastMessageList, { items: failures.map((report) =>
        toastMessage("extensions.updates.failureEntry", {
          name: items.find((item) => item.id === report.definitionId)?.name ?? report.definitionId,
          message: report.message,
        })) }),
    });
    else if (result.length > 0 && result.every((report) => report.status === "current")) {
      toast({ kind: "success", title: toastMessage("extensions.updates.allUpToDate") });
    }
  };
  const update = (selected: SkillUpdate[]) => updateSkills(ext, applies, updatable, selected,
    (ids) => setReports((previous) => previous.filter((report) => !ids.includes(report.definitionId))));
  return { reportMap, updatable, check, update };
}
