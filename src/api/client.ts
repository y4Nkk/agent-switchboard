/*
 * Typed client boundary. This module is the ONLY frontend file that talks to
 * the Tauri backend; components consume these typed wrappers and never build
 * configuration text or filesystem paths themselves.
 * (Enforced by boundary.test.ts.)
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type AppKind = "codex" | "claude";
export type PatchValue = boolean | string | number | PatchValue[];
export type RouteMode = "official" | "custom";

export interface CodexModelSettings {
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
  name: string;
  model: string | null;
  baseUrl: string | null;
  apiKey: string;
  modelOptions: ModelOptions | null;
  /** Local-only note; never written into any client configuration. */
  notes?: string | null;
  /** Provider homepage, used for navigation only. */
  websiteUrl?: string | null;
}

/** Every profile routes to a custom endpoint; official login is not a
 * profile kind (user decision 2026-08-28). */
export interface ProviderDraft {
  app: AppKind;
  name: string;
  model: string | null;
  baseUrl: string | null;
  apiKey: string;
  modelOptions: ModelOptions | null;
  notes?: string | null;
  websiteUrl?: string | null;
}

export interface PatchEntry {
  key: string;
  /** null removes the key's line from the target file. */
  value: PatchValue | null;
}

export interface CommonConfigPatch {
  app: AppKind;
  entries: PatchEntry[];
}

/** One official general-config toggle with the file's current line state. */
export interface ToggleState {
  key: string;
  label: string;
  line: string;
  /** The value the checked line carries (e.g. false for spinnerTipsEnabled). */
  applied: boolean;
  /** Whether the target file currently carries the applied line. */
  value: boolean;
  group: string;
}

/** One selectable value of a multi-detent general setting. */
export interface ChoiceOption {
  value: string;
  label: string;
}

/** One multi-detent general setting (e.g. reasoning effort, sandbox mode). */
export interface ChoiceState {
  key: string;
  label: string;
  group: string;
  /** "slider" renders the detent slider; "segment" renders pill segments. */
  control: "slider" | "segment";
  options: ChoiceOption[];
  /** Raw scalar at the key; null = line absent. May be outside the options. */
  value: string | null;
}

export interface RouteState {
  app: AppKind;
  routeMode: RouteMode;
  providerName: string | null;
  model: string | null;
  baseUrl: string | null;
  apiKey: string;
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
  | { kind: "matchesSettings"; at: string }
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
  warnings: string[];
  backupDir: string;
}

export interface FilePreview {
  preview: SwitchPreview;
  contentHash: string;
  renderedHash: string;
  /** Redacted candidate file text for the pretty-printed file view. */
  content: string;
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

export interface CcSwitchSkip {
  key: string;
  appType: string;
  name: string;
  reason: string;
}

export interface CcSwitchScanItem {
  key: string;
  app: AppKind;
  draft: ProviderDraft;
  warnings: string[];
  existing: boolean;
}

export interface CcSwitchScan {
  dbPath: string;
  providers: CcSwitchScanItem[];
  skipped: CcSwitchSkip[];
}

export interface CcSwitchImportOutcome {
  imported: ProviderProfile[];
  skippedExisting: string[];
  notImported: CcSwitchSkip[];
}

/** Read-only local session metadata. The backend never exposes source paths. */
export interface SessionMeta {
  app: AppKind;
  sessionId: string;
  title: string;
  summary: string;
  projectDir: string | null;
  createdAt: string | null;
  lastActiveAt: string | null;
  resumeCommand: string;
}

export interface SessionMessage {
  role: string;
  content: string;
  at: string | null;
}

export interface SessionIssue {
  app: AppKind;
  message: string;
}

export interface SessionScan {
  sessions: SessionMeta[];
  issues: SessionIssue[];
}

/** Result of starting a supported CLI's resume command in a new terminal. */
export interface SessionResume {
  command: string;
  usedProjectDir: boolean;
}

export function getConfigStatus(): Promise<ConfigFileStatus[]> {
  return invoke<ConfigFileStatus[]>("config_status");
}

export function listProfiles(): Promise<ProviderProfile[]> {
  return invoke<ProviderProfile[]>("list_profiles");
}

export function resetProfileStore(confirmWrite: boolean): Promise<void> {
  return invoke<void>("reset_profile_store", { confirmWrite });
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

export function reorderProfiles(target: AppKind, orderedIds: string[]): Promise<ProviderProfile[]> {
  return invoke<ProviderProfile[]>("reorder_profiles", { target, orderedIds });
}

export function importDiscoveredProfile(target: AppKind): Promise<ProviderProfile> {
  return invoke<ProviderProfile>("import_discovered_profile", { target });
}

export function scanCcswitch(): Promise<CcSwitchScan> {
  return invoke<CcSwitchScan>("scan_ccswitch");
}

export function importCcswitchProfiles(keys: string[]): Promise<CcSwitchImportOutcome> {
  return invoke<CcSwitchImportOutcome>("import_ccswitch_profiles", { keys });
}

export function getCommon(app: AppKind): Promise<CommonConfigPatch> {
  return invoke<CommonConfigPatch>("get_common", { target: app });
}

export function setCommon(app: AppKind, patch: CommonConfigPatch): Promise<void> {
  return invoke<void>("set_common", { target: app, patch });
}

export function getCommonToggles(app: AppKind): Promise<ToggleState[]> {
  return invoke<ToggleState[]>("common_toggles", { target: app });
}

/** The choice catalog for one client: section order plus the choices. */
export interface CommonChoicesState {
  groups: string[];
  choices: ChoiceState[];
}

export function getCommonChoices(app: AppKind): Promise<CommonChoicesState> {
  return invoke<CommonChoicesState>("common_choices", { target: app });
}

export function previewCommon(app: AppKind): Promise<FilePreview> {
  return invoke<FilePreview>("preview_common", { target: app });
}

/** Writes the general overlay through the executor's safe transaction. */
export function applyCommon(
  app: AppKind,
  patch: CommonConfigPatch,
  confirmWrite: boolean,
): Promise<SwitchOutcome> {
  return invoke<SwitchOutcome>("apply_common", { target: app, patch, confirmWrite });
}

export type CloseBehavior = "hideToTray" | "exit";
export type ThemePreference = "system" | "light" | "dark";
export type MotionPreference = "system" | "reduce";

/** Application-runtime desktop preferences; separate from client config. */
export interface AppSettings {
  closeBehavior: CloseBehavior;
  theme: ThemePreference;
  motion: MotionPreference;
  alwaysOnTop: boolean;
  hardwareAcceleration: boolean;
}

export function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_app_settings");
}

export function setAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("set_app_settings", { settings });
}

/** Dev-machine debug affordance: toggles the WebView inspector. */
export function toggleDevtools(): Promise<void> {
  return invoke<void>("toggle_devtools");
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

export function openBackupDir(): Promise<void> {
  return invoke<void>("open_backup_dir");
}

export function probeEndpoint(url: string): Promise<ProbeResult> {
  return invoke<ProbeResult>("probe_endpoint", { url });
}

/** Model ids from the provider's OpenAI-compatible /v1/models endpoint. */
export function fetchProviderModels(baseUrl: string, apiKey: string): Promise<string[]> {
  return invoke<string[]>("fetch_provider_models", { url: baseUrl, apiKey });
}

/** Result of one manual app-update check; informational only. */
export interface UpdateCheck {
  currentVersion: string;
  /** Release tag exactly as published, e.g. "v0.2.0". */
  latestVersion: string;
  updateAvailable: boolean;
  releaseUrl: string;
  checkedAt: string;
}

export function checkUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("check_update");
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

export function listSessions(): Promise<SessionScan> {
  return invoke<SessionScan>("list_sessions");
}

export function getSessionMessages(app: AppKind, sessionId: string): Promise<SessionMessage[]> {
  return invoke<SessionMessage[]>("get_session_messages", { app, sessionId });
}

export function resumeSession(app: AppKind, sessionId: string): Promise<SessionResume> {
  return invoke<SessionResume>("resume_session", { app, sessionId });
}

/* Integrated title bar window controls (undecorated window). These invoke
   app-owned commands in src-tauri (window_minimize / window_toggle_maximize /
   window_close / window_is_maximized), keeping all backend access inside this
   boundary. Maximize state is re-synced through window resize events. */
export function minimizeWindow(): Promise<void> {
  return invoke("window_minimize");
}

export function toggleMaximizeWindow(): Promise<void> {
  return invoke("window_toggle_maximize");
}

export function closeWindow(): Promise<void> {
  return invoke("window_close");
}

export function getWindowMaximized(): Promise<boolean> {
  return invoke<boolean>("window_is_maximized");
}

export function onWindowResized(handler: () => void): Promise<() => void> {
  return getCurrentWindow().onResized(() => handler());
}
