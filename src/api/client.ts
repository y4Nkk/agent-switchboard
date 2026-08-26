/*
 * Typed client boundary. This module is the ONLY frontend file that talks to
 * the Tauri backend; components consume these typed wrappers and never build
 * configuration text or filesystem paths themselves.
 * (Enforced by boundary.test.ts.)
 */
import { invoke } from "@tauri-apps/api/core";

export type AppKind = "codex" | "claude";
export type PatchValue = boolean | string | number | PatchValue[];
export type RouteMode = "official" | "custom";

export interface CodexModelSettings {
  reasoningEffort: string | null;
  reasoningSummary: string | null;
  verbosity: string | null;
  contextWindow: number | null;
}

export interface ClaudeModelSettings {
  haikuModel: string | null;
  sonnetModel: string | null;
  opusModel: string | null;
  availableModels: string[] | null;
}

export type ModelOptions =
  | ({ kind: "codex" } & CodexModelSettings)
  | ({ kind: "claude" } & ClaudeModelSettings);

export interface ProviderProfile {
  id: string;
  app: AppKind;
  mode: RouteMode;
  name: string;
  model: string | null;
  baseUrl: string | null;
  envKey: string | null;
  modelOptions: ModelOptions | null;
}

export interface ProviderDraft {
  app: AppKind;
  mode: RouteMode;
  name: string;
  model: string | null;
  baseUrl: string | null;
  envKey: string | null;
  modelOptions: ModelOptions | null;
}

export interface PatchEntry {
  key: string;
  value: PatchValue;
}

export interface CommonConfigPatch {
  app: AppKind;
  entries: PatchEntry[];
}

export interface RouteState {
  app: AppKind;
  routeMode: RouteMode;
  providerName: string | null;
  model: string | null;
  baseUrl: string | null;
  envKey: string | null;
  wireApi: string | null;
  codexModelOptions: CodexModelSettings | null;
  haikuModel: string | null;
  sonnetModel: string | null;
  opusModel: string | null;
  availableModels: string[] | null;
  scopeWarnings: string[];
}

export interface SwitchLog {
  app: AppKind;
  profileId: string | null;
  profileName: string | null;
  contentHash: string;
  backupId: string;
  at: string;
}

export type MatchStatus =
  | { kind: "matchesProfile"; profileId: string; profileName: string }
  | { kind: "profileChanged"; profileName: string }
  | { kind: "restoredBackup"; at: string }
  | { kind: "externallyModified"; at: string }
  | { kind: "unmanaged" }
  | { kind: "unknown" };

export interface ProbeResult {
  url: string;
  reachable: boolean;
  status: number | null;
  latencyMs: number | null;
  error: string | null;
  at: string;
}

export type LockStatus =
  | { state: "free" }
  | { state: "held"; pid?: number | null; processName?: string | null; acquiredAt?: string | null }
  | { state: "stale"; pid?: number | null; processName?: string | null; acquiredAt?: string | null }
  | { state: "indeterminate"; reason: string };

export interface KeyChange {
  key: string;
  kind: "set" | "remove";
  before: string | null;
  after: string | null;
}

export interface SwitchPreview {
  app: AppKind;
  target: string;
  changes: KeyChange[];
  preserved: string[];
  warnings: string[];
  backupDir: string;
}

export interface FilePreview {
  preview: SwitchPreview;
  contentHash: string;
  renderedHash: string;
}

export interface BackupRecord {
  id: string;
  app: AppKind;
  targetPath: string;
  backupPath: string;
  createdAt: string;
  contentHash: string;
  targetExisted: boolean;
  reason: string;
}

export interface ConfigFileStatus {
  app: AppKind;
  path: string;
  exists: boolean;
  syntaxOk: boolean;
  route: RouteState | null;
  readError: string | null;
  matchStatus: MatchStatus;
  lastSwitch: SwitchLog | null;
}

export type RecoveryOutcome =
  | { outcome: "not_needed" }
  | { outcome: "restored"; backup: BackupRecord }
  | { outcome: "restore_failed"; reason: string; backupPath: string };

export interface SwitchOutcome {
  lock: LockStatus;
  acquiredAt: string;
  changed: string[];
  warnings: string[];
  backup: BackupRecord;
  preview: SwitchPreview;
  recovery: RecoveryOutcome;
  finalHash: string;
}

export interface RestoreOutcome {
  preRestoreBackup: BackupRecord;
  restoredHash: string;
  warnings: string[];
}

export interface RecoveryEntry {
  lockPath: string;
  removedHolderPid: number | null;
  at: string;
}

export interface CommandError {
  code: string;
  message: string;
}

export type DiscoveredState =
  | { kind: "missing" }
  | { kind: "readError"; message: string }
  | { kind: "parseError"; message: string; line: number | null }
  | { kind: "ok"; route: RouteState; managed: boolean; warnings: string[]; importable: boolean };

export interface DiscoveredFile {
  app: AppKind;
  path: string;
  exists: boolean;
  state: DiscoveredState;
}

export interface ImportProposal {
  app: AppKind;
  draft: ProviderDraft;
  basis: string;
}

export interface DiscoveryReport {
  codex: DiscoveredFile;
  claude: DiscoveredFile;
  importProposals: ImportProposal[];
}

export function getConfigStatus(): Promise<ConfigFileStatus[]> {
  return invoke<ConfigFileStatus[]>("config_status");
}

export function listProfiles(): Promise<ProviderProfile[]> {
  return invoke<ProviderProfile[]>("list_profiles");
}

export function createProfile(draft: ProviderDraft): Promise<ProviderProfile> {
  return invoke<ProviderProfile>("create_profile", { draft });
}

export function updateProfile(profileId: string, draft: ProviderDraft): Promise<ProviderProfile> {
  return invoke<ProviderProfile>("update_profile", { profileId, draft });
}

export function deleteProfile(profileId: string): Promise<void> {
  return invoke<void>("delete_profile", { profileId });
}

export function importDiscoveredProfile(target: AppKind): Promise<ProviderProfile> {
  return invoke<ProviderProfile>("import_discovered_profile", { target });
}

export function getCommon(app: AppKind): Promise<CommonConfigPatch> {
  return invoke<CommonConfigPatch>("get_common", { target: app });
}

export function setCommon(app: AppKind, patch: CommonConfigPatch): Promise<void> {
  return invoke<void>("set_common", { target: app, patch });
}

export function previewSwitch(profileId: string): Promise<FilePreview> {
  return invoke<FilePreview>("preview_switch", { profileId });
}

export function executeSwitch(
  profileId: string,
  expectedHash: string,
  expectedRenderedHash: string,
  confirmWrite: boolean,
): Promise<SwitchOutcome> {
  return invoke<SwitchOutcome>("execute_switch", {
    profileId,
    expectedHash,
    expectedRenderedHash,
    confirmWrite,
  });
}

export function listBackups(): Promise<BackupRecord[]> {
  return invoke<BackupRecord[]>("list_backups");
}

export function restoreBackup(backupId: string, confirmWrite: boolean): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>("restore_backup", { backupId, confirmWrite });
}

export function undoLastSwitch(target: AppKind, confirmWrite: boolean): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>("undo_last_switch", { target, confirmWrite });
}

export function backupDiff(backupId: string): Promise<KeyChange[]> {
  return invoke<KeyChange[]>("backup_diff", { backupId });
}

export function probeEndpoint(url: string): Promise<ProbeResult> {
  return invoke<ProbeResult>("probe_endpoint", { url });
}

export function getLockStatus(app: AppKind): Promise<LockStatus> {
  return invoke<LockStatus>("lock_status", { target: app });
}

export function recoverStaleLock(app: AppKind): Promise<RecoveryEntry> {
  return invoke<RecoveryEntry>("recover_stale_lock", { target: app });
}

export function discoverLocal(): Promise<DiscoveryReport> {
  return invoke<DiscoveryReport>("discover_local");
}
