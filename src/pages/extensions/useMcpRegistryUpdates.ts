import { createElement, useState } from "react";
import { applyMcpRegistryUpdate, checkMcpRegistryUpdates, type McpRegistryUpdate, type McpRegistryUpdateReport } from "../../api/extensions/registry";
import { definitionUpdatePreparation, refreshLibraryWrite } from "../../app/extensions/extension-ops";
import type { useExtensions } from "../../app/useExtensions";
import { ToastMessageList } from "../../components/ToastContent";
import { toast, toastMessage } from "../../components/use-toast";

export function useMcpRegistryUpdates(ext: ReturnType<typeof useExtensions>) {
  const [reports, setReports] = useState<McpRegistryUpdateReport[]>([]);
  const items = ext.workspace?.items ?? [];
  const itemsById = new Map(items.map((item) => [item.id, item]));
  const reportMap = new Map(reports.filter((report) => itemsById.get(report.definitionId)?.revision === report.revision)
    .map((report) => [report.definitionId, report]));
  const check = async (ids: string[]) => {
    if (ids.length === 0) return;
    const checked = await ext.runExclusive(() => checkMcpRegistryUpdates(ids));
    if (!checked) return;
    setReports((previous) => [...checked, ...previous.filter((report) => !ids.includes(report.definitionId))]);
    const failures = checked.filter((report) => report.status === "failed");
    const available = checked.filter((report) => report.status === "available").length;
    const manual = checked.filter((report) => report.status === "manual").length;
    const summary = available + manual > 0 ? toastMessage("extensions.registry.updateSummary", { available, manual }) : null;
    if (failures.length > 0) {
      const details = failures.map((report) => toastMessage("extensions.updates.failureEntry", {
        name: itemsById.get(report.definitionId)?.name ?? report.definitionId,
        message: report.message,
      }));
      if (summary) details.unshift(summary);
      toast({
        kind: "warning", title: toastMessage("extensions.registry.checkFailed", { count: failures.length }),
        description: createElement(ToastMessageList, { items: details }),
      });
    } else if (summary) {
      toast({ kind: "info", title: summary });
    } else if (checked.length > 0) {
      toast({ kind: "success", title: toastMessage("extensions.registry.upToDate") });
    }
  };
  const apply = (report: McpRegistryUpdate) => ext.runExclusive(async () => {
    const saved = await applyMcpRegistryUpdate(report);
    setReports((previous) => previous.filter((entry) => entry.definitionId !== report.definitionId));
    if (report.action === "associateCurrent") {
      await refreshLibraryWrite(ext.refresh, { title: toastMessage("extensions.registry.associated") });
      return { kind: "associated" } as const;
    }
    return { kind: "endpointReplaced", preparation: definitionUpdatePreparation(saved, await ext.refresh()) } as const;
  });
  return { reportMap, check, apply };
}
