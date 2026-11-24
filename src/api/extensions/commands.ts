import { invoke } from "../client";
import type { AppKind } from "../shared";
import type {
  ApplyOutcome,
  ExtensionDiscovery,
  ExtensionDraft,
  ExtensionMutation,
  ExtensionOperationRecord,
  ExtensionPlanView,
  ExtensionTarget,
  ExtensionsWorkspace,
  McpCheckStatus,
  McpEditRequest,
  McpEditViewEnvelope,
  PlanRequest,
  ProjectRegistration,
  SkillCandidateDto,
  SkillDependency,
  SkillManifest,
  SkillUpdateReport,
} from "./types";

export function listExtensions(): Promise<ExtensionsWorkspace> {
  return invoke<ExtensionsWorkspace>("list_extensions");
}

export function recoverExtensionTransactions(): Promise<string[]> {
  return invoke<string[]>("recover_extension_transactions");
}

export function discoverExtensions(): Promise<ExtensionDiscovery> {
  return invoke<ExtensionDiscovery>("discover_extensions");
}

export function saveExtension(draft: ExtensionDraft): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("save_extension", { draft });
}

export function getMcpEditView(definitionId: string): Promise<McpEditViewEnvelope> {
  return invoke<McpEditViewEnvelope>("get_mcp_edit_view", { definitionId });
}

export function updateMcpDefinition(
  definitionId: string,
  edit: McpEditRequest,
): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("update_mcp_definition", { definitionId, edit });
}

export function deleteExtension(id: string, confirmWrite: boolean): Promise<void> {
  return invoke<void>("delete_extension", { id, confirmWrite });
}

export function registerProject(root: string): Promise<ProjectRegistration> {
  return invoke<ProjectRegistration>("register_project", { root });
}

export function scanLocalSkillSource(root: string): Promise<SkillCandidateDto[]> {
  return invoke<SkillCandidateDto[]>("scan_local_skill_source", { root });
}

export function resolveSkillSource(
  repo: string,
  subpath = "",
  refName: string | null = null,
): Promise<SkillCandidateDto[]> {
  return invoke<SkillCandidateDto[]>("resolve_skill_source", { repo, subpath, refName });
}

export function importSkillCandidate(
  candidateId: string,
  name: string,
  hostScoped: AppKind | null,
): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("import_skill_candidate", { candidateId, name, hostScoped });
}

export function importDiscoveredSkill(observationId: string): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("import_discovered_skill", { observationId });
}

export function importDiscoveredMcp(observationId: string): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("import_discovered_mcp", { observationId });
}

/** Confirms a takeover. The native entry or directory is recorded as the
 * ownership baseline; no client file is written by the takeover itself. */
export function takeoverDiscoveredExtension(observationId: string): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("takeover_discovered_extension", {
    observationId,
    confirmWrite: true,
  });
}

/** Result of importing one portable package: the created definition plus
 * the credential slots the user must re-bind on this machine. */
export interface PortableImportReport {
  definition: ExtensionMutation;
  missingEnvSlots: string[];
  warnings: string[];
}

export function exportExtensionPortable(definitionId: string, targetPath: string): Promise<void> {
  return invoke<void>("export_extension_portable", { definitionId, targetPath });
}

export function importExtensionPortable(packagePath: string): Promise<PortableImportReport> {
  return invoke<PortableImportReport>("import_extension_portable", {
    packagePath,
    confirmWrite: true,
  });
}

export function checkSkillUpdates(definitionIds: string[]): Promise<SkillUpdateReport[]> {
  return invoke<SkillUpdateReport[]>("check_skill_updates", { definitionIds });
}

/** Pins or unpins one Skill binding's content version; a library-only
 * record change with no client write. */
export function setBindingLock(bindingId: string, locked: boolean): Promise<void> {
  return invoke<void>("set_binding_lock", { bindingId, locked });
}

export function setSkillVersion(
  definitionId: string,
  expectedRevision: number,
  change: { kind: "upstream"; candidateId: string } | { kind: "restore"; digest: string },
): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("set_skill_version", { definitionId, expectedRevision, change });
}

/** One file of a local skill's content version as loaded for the editor.
 * Binary or oversized files carry no text and are carried over unchanged
 * on save. */
export interface SkillEditorFile {
  relativePath: string;
  text: string | null;
  size: number;
}

export interface SkillEditorView {
  id: string;
  revision: number;
  contentDigest: string;
  manifest: SkillManifest;
  /** Whether update_skill_files accepts edits; sourced or host-scoped
   * skills must be forked into a local copy first. */
  editable: boolean;
  files: SkillEditorFile[];
}

export interface LocalSkillDraft {
  name: string;
  description: string;
}

export interface SkillFileEdit {
  relativePath: string;
  text: string;
}

export interface SkillFilesUpdate {
  expectedRevision: number;
  expectedDigest: string;
  files: SkillFileEdit[];
}

export interface SkillDependenciesUpdate {
  expectedRevision: number;
  dependencies: SkillDependency[];
}

export interface SkillVersion {
  digest: string;
  fileCount: number;
  totalBytes: number;
  isCurrent: boolean;
}

export function createLocalSkill(draft: LocalSkillDraft): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("create_local_skill", { draft });
}

export function forkLocalSkill(definitionId: string): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("fork_local_skill", { definitionId });
}

export function getSkillEditor(definitionId: string): Promise<SkillEditorView> {
  return invoke<SkillEditorView>("get_skill_editor", { definitionId });
}

export function updateSkillFiles(
  definitionId: string,
  update: SkillFilesUpdate,
): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("update_skill_files", { definitionId, update });
}

export function listSkillVersions(definitionId: string): Promise<SkillVersion[]> {
  return invoke<SkillVersion[]>("list_skill_versions", { definitionId });
}

export function updateSkillDependencies(
  definitionId: string,
  update: SkillDependenciesUpdate,
): Promise<ExtensionMutation> {
  return invoke<ExtensionMutation>("update_skill_dependencies", { definitionId, update });
}

export function putExtensionSecret(value: string, purpose: string): Promise<{ reference: string }> {
  return invoke<{ reference: string }>("put_extension_secret", { value, purpose });
}

export function prepareExtensionPlan(request: PlanRequest): Promise<ExtensionPlanView> {
  return invoke<ExtensionPlanView>("prepare_extension_plan", { request });
}

/** Prepares the one-click repair batch for the latest scan's auto-repairable
 * diagnostics. The preview it returns still goes through the shared
 * confirm-then-apply flow; nothing is written here. */
export function prepareExtensionRepair(
  scanId: string,
  diagnosticIds: string[],
): Promise<ExtensionPlanView> {
  return invoke<ExtensionPlanView>("prepare_extension_repair", {
    request: { scanId, diagnosticIds },
  });
}

export function applyExtensionPlan(
  planId: string,
  confirmWrite: boolean,
): Promise<ApplyOutcome> {
  return invoke<ApplyOutcome>("apply_extension_plan", { planId, confirmWrite });
}

export function getExtensionOperation(operationId: string): Promise<ExtensionOperationRecord | null> {
  return invoke<ExtensionOperationRecord | null>("get_extension_operation", { operationId });
}

export function prepareExtensionRestore(operationId: string): Promise<ExtensionPlanView> {
  return invoke<ExtensionPlanView>("prepare_extension_restore", { operationId });
}

export function checkMcpConnection(
  definitionId: string,
  definitionRevision: number,
  target: ExtensionTarget,
  confirm: boolean,
): Promise<{ checkId: string }> {
  return invoke<{ checkId: string }>("check_mcp_connection", { definitionId, definitionRevision, target, confirm });
}

export function getMcpCheck(checkId: string): Promise<McpCheckStatus> {
  return invoke<McpCheckStatus>("get_mcp_check", { checkId });
}

export function cancelMcpCheck(checkId: string): Promise<boolean> {
  return invoke<boolean>("cancel_mcp_check", { checkId });
}
