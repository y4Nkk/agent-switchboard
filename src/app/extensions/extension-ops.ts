import type {
  CommandError,
  ExtensionMutation,
  ExtensionsWorkspace,
} from "../../api/client";
import { toast, toastMessage, type ToastContent } from "../../components/use-toast";

export interface ExtensionsDeps {
  busy: boolean;
  setBusy: (busy: boolean) => void;
  clearError: () => void;
  onError: (error: CommandError) => void;
}

/** A committed definition is distinct from its verified deployment state. */
export interface SkillUpdatePreparation {
  definition: ExtensionMutation;
  deployment: "required" | "notRequired" | "unverified";
}

/** Fresh library snapshot and per-item failures after applying Skill updates. */
export interface SkillBatchAdvance {
  failed: Array<{ definitionId: string; error: unknown }>;
  workspace: ExtensionsWorkspace | null;
}

export type EditPreparation = SkillUpdatePreparation;

/** Serialises one mutation into the shared operation frame; `null` marks a
 * rejected or failed action whose error already surfaced. */
export type ExclusiveRunner = <T>(action: () => Promise<T>) => Promise<T | null>;

/** Reads may run during a write so refreshed editors can load new revisions. */
export type ExtensionReader = <T>(action: () => Promise<T>) => Promise<T | null>;

/** Re-reads the extension workspace after a mutation. */
export type WorkspaceRefresher = () => Promise<ExtensionsWorkspace | null>;

export async function refreshLibraryWrite(
  refresh: WorkspaceRefresher,
  notification: {
    title: ToastContent;
    description?: ToastContent;
    kind?: "info" | "success";
  },
): Promise<ExtensionsWorkspace | null> {
  const workspace = await refresh();
  toast(workspace === null
    ? {
        kind: "warning",
        title: toastMessage("extensions.ops.refreshFailedTitle"),
        description: toastMessage("extensions.ops.refreshFailedBody"),
      }
    : { kind: "success", ...notification });
  return workspace;
}

export function definitionUpdatePreparation(
  definition: ExtensionMutation,
  workspace: ExtensionsWorkspace | null,
): SkillUpdatePreparation {
  const item = workspace?.items.find((entry) => entry.id === definition.id);
  if (!item) {
    toast({
      kind: "warning",
      title: toastMessage("extensions.ops.savedUnconfirmed"),
      description: toastMessage("extensions.ops.savedUnconfirmedBody"),
    });
    return { definition, deployment: "unverified" };
  }
  const required = item.kind === "mcp"
    ? item.bindings.length > 0
    : item.bindings.some((binding) => binding.desired === "enabled" && !binding.lockedDigest);
  if (!required) toast({ kind: "success", title: toastMessage("extensions.ops.savedNoDeployment") });
  return { definition, deployment: required ? "required" : "notRequired" };
}
