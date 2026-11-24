import type { AppKind } from "../shared";
import type { McpDefinition, McpMetadata } from "./mcp";
export type { CodexServerOptions, FieldEdit, McpDefinition, McpEditRequest, McpEditView,
  McpEditViewEnvelope, McpFieldEdits, McpMetadata, SecretSlot, SecretSlotView } from "./mcp";

/* ---------------------------------------------------------------- Extensions
 * workspace (Skills / MCP). Types mirror the serde camelCase DTOs in
 * crates/asb-core/src/extensions; commands live in
 * src-tauri/src/commands/extensions.rs. */

export type ExtensionKind = "skill" | "mcp";

export type ExtensionTarget =
  | { scope: "app"; client: AppKind }
  | { scope: "projectShared"; client: AppKind; projectId: string }
  | { scope: "projectPrivate"; client: AppKind; projectId: string };

/** One value position that can carry sensitive material. */
export type SecretValue =
  | { mode: "envRef"; name: string }
  | { mode: "secretRef"; reference: string }
  | { mode: "plain"; value: string };

export interface SkillManifest {
  name: string;
  description?: string | null;
  license?: string | null;
  allowedTools?: string[] | null;
  unparsedKeys?: string[];
}

export interface SourceRef {
  sourceId: string;
  subpath: string;
  refName?: string | null;
  resolvedCommit?: string | null;
}

export interface SkillDependency {
  name: string;
  resourceId?: string | null;
}

export interface CompatibilityNote {
  code: string;
  message: string;
}

export interface SkillDefinition {
  contentDigest: string;
  manifest: SkillManifest;
  source?: SourceRef | null;
  hostScoped?: AppKind | null;
  compatibility?: CompatibilityNote[];
  dependencies?: SkillDependency[];
}

export type ExtensionPayload =
  | ({ kind: "skill" } & SkillDefinition)
  | ({ kind: "mcp" } & McpDefinition);

export interface ExtensionDefinitionEnvelope {
  schemaVersion: number;
  id: string;
  name: string;
  revision: number;
  createdAt: string;
  updatedAt: string;
  mcpMetadata?: McpMetadata | null;
}

export type ExtensionDefinition = ExtensionDefinitionEnvelope & ExtensionPayload;

export type DesiredState = "enabled" | "disabled";

export interface ExtensionBinding {
  schemaVersion: number;
  id: string;
  resourceId: string;
  target: ExtensionTarget;
  nativeKey?: string | null;
  deployName?: string | null;
  desired: DesiredState;
  lockedDigest?: string | null;
  lastAppliedRevision?: number | null;
  updatedAt: string;
}

export type FileState =
  | "notDeployed"
  | "inSync"
  | "pendingApply"
  | "externalChange"
  | "missing"
  | "unreadable";

export type BindingStatus = ExtensionBinding & {
  fileState: FileState;
  warnings: string[];
};

export type DependencyState = "bound" | "pendingConfiguration" | "targetUnsupported";

export interface DependencyStatus {
  name: string;
  resourceId: string | null;
  state: DependencyState;
}

/** Safe renderer projection of a credential position. This is intentionally
 * separate from SecretValue, which is accepted only when saving a draft. */
export type SecretValueView =
  | { mode: "envRef"; name: string }
  | { mode: "stored" }
  | { mode: "redacted" };

/** Only validated, commit-pinned GitHub provenance may expose links. */
export interface SkillSourceView {
  resolvedCommit?: string | null;
  repo?: string | null;
  subpath?: string | null;
  refName?: string | null;
  readmeUrl?: string | null;
}

type ExtensionListItemCommon = ExtensionDefinitionEnvelope & {
  bindings: BindingStatus[];
  dependencyStates: DependencyStatus[];
  /** Latest result per target for this definition revision, newest first. */
  checks: McpCheckResult[];
};

export type ExtensionListItem =
  | (ExtensionListItemCommon & {
      kind: "skill";
      contentDigest: string;
      manifest: SkillManifest;
      source: SkillSourceView | null;
      hostScoped: AppKind | null;
      compatibility: CompatibilityNote[];
      /** Declared dependencies with their library MCP links; states derive
       * per target at read time. */
      dependencies: SkillDependency[];
    })
  | (ExtensionListItemCommon & {
      kind: "mcp";
      transport: "stdio";
      command: string;
      argumentCount: number;
      env: Record<string, SecretValueView>;
    })
  | (ExtensionListItemCommon & {
      kind: "mcp";
      transport: "http" | "claudeSse" | "claudeWs";
      /** Always the stable redaction marker, never the upstream address. */
      url: string;
      headers: Record<string, SecretValueView>;
      bearer?: SecretValueView;
    });

/** The safe result from an extension-library mutation. */
export interface ExtensionMutation {
  id: string;
  name: string;
  revision: number;
}

export interface ProjectRegistration {
  schemaVersion: number;
  id: string;
  root: string;
  displayName: string;
  registeredAt: string;
}

export type ObservedOrigin =
  | { origin: "userRoot" }
  | { origin: "legacyRoot" }
  | { origin: "projectRoot"; projectId: string }
  | { origin: "managed" };

/** Backend-judged eligibility of one discovered row's actions. */
export interface ActionSupport {
  supported: boolean;
  /** An equivalent definition already exists in the library. */
  inLibrary?: boolean;
  reason?: string | null;
}

export interface ObservedActions {
  /** Copy the content into the library; the original stays independent. */
  import: ActionSupport;
  /** Start managing the current location, recording its original state. */
  takeover: ActionSupport;
  /** The library definition managing this row, when managed. */
  managedDefinitionId?: string | null;
}

export interface ObservedExtension {
  observationId: string;
  kind: ExtensionKind;
  client: AppKind;
  name: string;
  description?: string | null;
  origin: ObservedOrigin;
  managed: boolean;
  contentDigest?: string | null;
  transport?: string | null;
  actions: ObservedActions;
}

/** Stable machine code of one diagnostic; message text is display-only. */
export type DiagnosticCode =
  | "skillManifestMissing"
  | "skillFrontmatterMissing"
  | "skillFrontmatterInvalid"
  | "skillDirUnreadable"
  | "skillEntryLink"
  | "skillEntryUnsupported"
  | "skillRootUnreadable"
  | "skillRootEntryUnreadable"
  | "mcpDocumentUnreadable"
  | "mcpDocumentUnparsable"
  | "mcpCollectionInvalid"
  | "mcpEntryNotAnObject"
  | "mcpTransportMissing"
  | "mcpTransportConflicting"
  | "mcpTransportUnknown"
  | "mcpUnknownFields"
  | "mcpIgnoredField"
  | "managedTargetMissing"
  | "managedEntryMissing"
  | "managedTargetExternalChange"
  | "managedTargetUnreadable";

export type ExtensionDiagnosticRemediation =
  | { kind: "auto"; reason: string }
  | { kind: "manual"; reason: string }
  | { kind: "info" };

export type DiagnosticSubject =
  | { kind: "discoveryEntry"; observationId: string }
  | { kind: "managedBinding"; bindingId: string }
  | { kind: "scanLocation"; label: string; resourceKind: ExtensionKind };

export interface ExtensionDiagnostic {
  id: string;
  code: DiagnosticCode;
  client: AppKind;
  subject: DiagnosticSubject;
  message: string;
  remediation: ExtensionDiagnosticRemediation;
}

/** One discovery scan: a snapshot identity plus everything observed and
 * every problem found. A repair request must name this scan. */
export interface ExtensionDiscovery {
  scanId: string;
  scannedAt: string;
  observations: ObservedExtension[];
  diagnostics: ExtensionDiagnostic[];
}

export interface SkillCandidateDto {
  candidateId: string;
  digest: string;
  name: string;
  description: string | null;
  fileCount: number;
  diagnostics: string[];
  source?: SkillSourceView;
}

export type SkillFileChangeAction = "added" | "removed" | "modified";

export interface SkillFileChange {
  relativePath: string;
  action: SkillFileChangeAction;
}

/** One skill's source-freshness result. Batch checks keep going when a
 * single skill fails; that failure lands in `error` instead of aborting
 * the whole request. */
export type SkillUpdateReport = { definitionId: string; revision: number } & (
  | { status: "current" }
  | { status: "available"; candidateId: string; commit: string; digest: string; changedFiles: SkillFileChange[] }
  | { status: "failed"; message: string }
);

export type SkillUpdate = Extract<SkillUpdateReport, { status: "available" }>;

export type CapabilityVerification =
  | { verification: "verified"; clientVersion: string; verifiedOn: string }
  | { verification: "open"; condition: string };

export type CapabilityEntry = {
  code: string;
  resource: string;
  supported: boolean;
} & CapabilityVerification;

export interface ClientCapabilityReport {
  client: AppKind;
  entries: CapabilityEntry[];
}

export type McpCheckOutcome =
  | { kind: "passed"; tools: number; resources: number; prompts: number }
  | { kind: "partial"; error: string }
  | { kind: "failed"; classification: string; error: string }
  | { kind: "cancelled" }
  | { kind: "needsNativeConfirmation" };

export interface McpCheckResult {
  definitionId: string;
  definitionRevision: number;
  target: ExtensionTarget;
  checkedAt: string;
  protocolVersion?: string | null;
  outcome: McpCheckOutcome;
  durationMs: number;
  truncated: boolean;
}

export type McpCheckStatus =
  | { state: "running" }
  | { state: "completed"; result: McpCheckResult };

export type TargetOutcome =
  | { kind: "applied" }
  | { kind: "failed"; message: string }
  | { kind: "restored"; message: string }
  | { kind: "restoreFailed"; message: string; backup: string }
  | { kind: "skipped"; message: string };

export interface ExtensionTargetResult {
  target: ExtensionTarget;
  outcome: TargetOutcome;
  warnings?: string[];
}

export interface RollbackSummary {
  restored: string[];
  failed: string[];
}

export type PlanOperation =
  | "install"
  | "update"
  | "enable"
  | "disable"
  | "remove"
  | "restore"
  | "repair";

/** One resource's slice of a finished batch operation. */
export interface OperationResourceRecord {
  definitionId: string;
  definitionRevision: number;
  operation: PlanOperation;
  targets: ExtensionTargetResult[];
}

export interface ExtensionOperationRecord {
  schemaVersion: number;
  id: string;
  createdAt: string;
  finishedAt?: string | null;
  resources: OperationResourceRecord[];
  rollback?: RollbackSummary | null;
}

export interface PlanChangeView {
  pointer: string;
  before?: string | null;
  after?: string | null;
}

export type FileAction = "deploy" | "remove";

export interface FileChangeView {
  relativePath: string;
  action: FileAction;
}

export interface PlannedTargetView {
  target: ExtensionTarget;
  warnings: string[];
  changes: PlanChangeView[];
  files?: FileChangeView[];
  writesSensitiveConnectionData: boolean;
  /** 该目标将替换一个未被本应用接管的同名原生条目，需要显式确认。 */
  adoptsNativeEntry: boolean;
}

export interface PlannedOperationView {
  definitionId: string;
  definitionRevision: number;
  operation: PlanOperation;
  targets: PlannedTargetView[];
}

/** The redacted batch plan preview, grouped by resource operation. */
export interface ExtensionPlanView {
  planId: string;
  createdAt: string;
  expiresAt: string;
  operations: PlannedOperationView[];
}

export interface ExtensionsWorkspace {
  generation: number;
  items: ExtensionListItem[];
  projects: ProjectRegistration[];
  history: ExtensionOperationRecord[];
  capabilities: ClientCapabilityReport[];
  recoveryRequired: string[];
}

export interface ExtensionDraft {
  name: string;
  mcpMetadata?: McpMetadata | null;
  payload: ExtensionPayload;
}

/** One typed resource operation inside a batch plan request: a
 * single-resource request is a batch of one. */
export interface PlanRequestOperation {
  operation: PlanOperation;
  definitionId?: string | null;
  bindingId?: string | null;
  targets?: ExtensionTarget[];
  sharedSettings?: boolean | null;
}

export interface PlanRequest {
  operations: PlanRequestOperation[];
}

export interface ApplyOutcome {
  record: ExtensionOperationRecord | null;
  rejected: string | null;
  rolledBack: boolean;
}
