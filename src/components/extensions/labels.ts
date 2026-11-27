import type {
  AppKind,
  ExtensionDiagnosticRemediation,
  ExtensionListItem,
  ExtensionTarget,
  FileState,
  PlanOperation,
  TargetOutcome,
} from "../../api/client";
import type { MessageKey } from "../../i18n";
import { tr } from "../../i18n/current";
import { clientName } from "../../lib/client-name";

export const FILE_STATE_LABELS: Record<FileState, MessageKey> = {
  notDeployed: "extensions.fileState.notDeployed",
  inSync: "extensions.fileState.inSync",
  pendingApply: "extensions.fileState.pendingApply",
  externalChange: "extensions.fileState.externalChange",
  missing: "extensions.fileState.missing",
  unreadable: "extensions.fileState.unreadable",
};

export const OPERATION_LABELS: Record<PlanOperation, MessageKey> = {
  install: "extensions.operation.install",
  update: "extensions.operation.update",
  enable: "extensions.operation.enable",
  disable: "extensions.operation.disable",
  remove: "extensions.operation.remove",
  restore: "extensions.operation.restore",
  repair: "extensions.operation.repair",
};

/** Renderer-safe label keys for diagnostic problem codes. */
export const DIAGNOSTIC_CODE_LABELS: Record<string, MessageKey> = {
  skillManifestMissing: "extensions.diagnostic.skillManifestMissing",
  skillFrontmatterMissing: "extensions.diagnostic.skillFrontmatterMissing",
  skillFrontmatterInvalid: "extensions.diagnostic.skillFrontmatterInvalid",
  skillDirUnreadable: "extensions.diagnostic.skillDirUnreadable",
  skillEntryLink: "extensions.diagnostic.skillEntryLink",
  skillEntryUnsupported: "extensions.diagnostic.skillEntryUnsupported",
  skillRootUnreadable: "extensions.diagnostic.skillRootUnreadable",
  skillRootEntryUnreadable: "extensions.diagnostic.skillRootEntryUnreadable",
  mcpDocumentUnreadable: "extensions.diagnostic.mcpDocumentUnreadable",
  mcpDocumentUnparsable: "extensions.diagnostic.mcpDocumentUnparsable",
  mcpCollectionInvalid: "extensions.diagnostic.mcpCollectionInvalid",
  mcpEntryNotAnObject: "extensions.diagnostic.mcpEntryNotAnObject",
  mcpTransportMissing: "extensions.diagnostic.mcpTransportMissing",
  mcpTransportConflicting: "extensions.diagnostic.mcpTransportConflicting",
  mcpTransportUnknown: "extensions.diagnostic.mcpTransportUnknown",
  mcpUnknownFields: "extensions.diagnostic.mcpUnknownFields",
  mcpIgnoredField: "extensions.diagnostic.mcpIgnoredField",
  managedTargetMissing: "extensions.diagnostic.managedTargetMissing",
  managedEntryMissing: "extensions.diagnostic.managedEntryMissing",
  managedTargetExternalChange: "extensions.fileState.externalChange",
  managedTargetUnreadable: "extensions.diagnostic.managedTargetUnreadable",
};

export const REMEDIATION_LABELS: Record<ExtensionDiagnosticRemediation["kind"], MessageKey> = {
  auto: "extensions.remediation.auto",
  manual: "extensions.remediation.manual",
  info: "extensions.remediation.info",
};

export const TRANSPORT_LABELS: Record<string, MessageKey> = {
  stdio: "extensions.transport.stdio",
  http: "extensions.transport.http",
  claudeSse: "extensions.transport.claudeSse",
  claudeWs: "extensions.transport.claudeWs",
};

/** Worst-state-first ordering: the summary names the state that needs
 * attention before the reassuring ones. */
const FILE_STATE_SEVERITY: Array<ExtensionListItem["bindings"][number]["fileState"]> = [
  "unreadable",
  "externalChange",
  "missing",
  "pendingApply",
  "notDeployed",
  "inSync",
];

/** One line of text for an item's deployment state on one client; the row
 * toggles carry it as their accessible name so state never rides on colour
 * alone. */
export function clientSummary(item: ExtensionListItem, client: AppKind): string {
  const rows = item.bindings.filter((binding) => binding.target.client === client);
  if (rows.length === 0) return tr(FILE_STATE_LABELS.notDeployed);
  const worst =
    FILE_STATE_SEVERITY.find((state) => rows.some((binding) => binding.fileState === state)) ?? "inSync";
  const enabled = rows.filter((binding) => binding.desired === "enabled").length;
  if (enabled > 0 && enabled < rows.length)
    return `${tr(FILE_STATE_LABELS[worst])} · ${tr("extensions.summary.partialEnabled")}`;
  return enabled === 0
    ? `${tr(FILE_STATE_LABELS[worst])} · ${tr("extensions.summary.disabled")}`
    : tr(FILE_STATE_LABELS[worst]);
}

export function targetLabel(target: ExtensionTarget, projectNames?: ReadonlyMap<string, string>): string {
  const client = clientName(target.client);
  if (target.scope === "app") return tr("extensions.target.user", { client });
  const project = projectNames?.get(target.projectId);
  const scope = target.scope === "projectShared"
    ? tr("extensions.target.projectShared")
    : tr("extensions.target.projectPrivate");
  return project
    ? tr("extensions.target.projectNamed", { project, client, scope })
    : tr("extensions.target.clientScope", { client, scope });
}

/** Encodes one target for the Select control; the client stays a plain
 * string because Select options are string-keyed. */
export function targetValue(target: ExtensionTarget): string {
  if (target.scope === "app") return `app:${target.client}`;
  return `${target.scope}:${target.client}:${target.projectId}`;
}

export function parseTargetValue(value: string): ExtensionTarget | null {
  const [scope, client, projectId] = value.split(":");
  if (scope === "app" && (client === "codex" || client === "claude")) {
    return { scope: "app", client };
  }
  if (
    (scope === "projectShared" || scope === "projectPrivate") &&
    (client === "codex" || client === "claude") &&
    projectId
  ) {
    return { scope, client, projectId };
  }
  return null;
}

export function outcomeText(outcome: TargetOutcome): string {
  switch (outcome.kind) {
    case "applied":
      return tr("extensions.outcome.applied");
    case "failed":
      return tr("extensions.outcome.failed", { message: outcome.message });
    case "restored":
      return tr("extensions.outcome.restored", { message: outcome.message });
    case "restoreFailed":
      return tr("extensions.outcome.restoreFailed", { message: outcome.message, backup: outcome.backup });
    case "skipped":
      return tr("extensions.outcome.skipped", { message: outcome.message });
  }
}
