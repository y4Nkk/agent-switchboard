/*
 * Typed client boundary. This module is the ONLY frontend file that talks to
 * the Tauri backend; components consume these typed wrappers and never build
 * configuration text or filesystem paths themselves.
 * (Enforced by boundary.test.ts.)
 */
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isBrowserDevelopment } from "../lib/runtime";

type InvokeArgs = Record<string, unknown>;

interface WebCommandResponse<T> {
  kind: "success" | "failure";
  result?: T;
  error?: CommandError;
}

/** In browser development, Vite proxies this call to the local Tauri helper
 * process. Desktop and test code keep the native Tauri invoke transport. */
async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  if (!isBrowserDevelopment) {
    return args === undefined ? tauriInvoke<T>(command) : tauriInvoke<T>(command, args);
  }

  let response: Response;
  try {
    response = await fetch("/api/invoke", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ command, args }),
    });
  } catch {
    throw {
      code: "web-backend-unavailable",
      message: "本机开发后端未就绪；请通过 npm run dev 启动应用",
    } satisfies CommandError;
  }

  const payload = (await response.json().catch(() => null)) as WebCommandResponse<T> | null;
  if (!response.ok || !payload) {
    throw {
      code: "web-backend-unavailable",
      message: "本机开发后端没有返回有效响应",
    } satisfies CommandError;
  }
  if (payload.kind === "failure") {
    throw payload.error;
  }
  return payload.result as T;
}

export type AppKind = "codex" | "claude";
export type PatchValue = boolean | string | number | PatchValue[];
export type RouteMode = "official" | "custom";

export interface CodexModelSettings {
  contextWindow: number | null;
}

export interface ClaudeModelSettings {
  primaryOneM: boolean;
  haikuModel: string | null;
  sonnetModel: string | null;
  sonnetOneM: boolean;
  opusModel: string | null;
  opusOneM: boolean;
  availableModels: string[] | null;
}

export type ModelOptions =
  | ({ kind: "codex" } & CodexModelSettings)
  | ({ kind: "claude" } & ClaudeModelSettings);

/** One declarative usage-balance query: a GET against the provider endpoint
 * with `{{baseUrl}}` / `{{apiKey}}` placeholders plus JSON Pointer paths. */
export interface DeclarativeUsageQuery {
  kind: "declarative";
  url: string;
  remainingPath?: string | null;
  usedPath?: string | null;
  totalPath?: string | null;
  /** Display unit for the extracted numbers, e.g. "USD". */
  unit?: string | null;
}

/** A self-authored JavaScript query. The source evaluates to `{ request,
 * extract }`; it is executed in the backend's restricted runtime. */
export interface ScriptUsageQuery {
  kind: "script";
  source: string;
}

/** The only persisted usage-query contract. `null` on a profile means the
 * optional feature is not configured. */
export type UsageQuery = DeclarativeUsageQuery | ScriptUsageQuery;

/** Numbers picked out of one usage-query response. */
export interface UsageSummary {
  remaining: number | null;
  used: number | null;
  total: number | null;
  unit: string | null;
  at: string;
}

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
  /** Application-side usage-balance query; never written into client config. */
  usageQuery?: UsageQuery | null;
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
  usageQuery?: UsageQuery | null;
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

/** One backend-resolved global instruction document. Its absolute path never
 * crosses the renderer boundary; the hash protects against stale saves. */
export interface GlobalPromptDocument {
  app: AppKind;
  fileName: string;
  content: string;
  contentHash: string;
  exists: boolean;
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

export type SwitchOperation = "switch" | "commonsettings";

export interface SwitchLog {
  app: AppKind;
  profileId: string | null;
  profileName: string | null;
  contentHash: string;
  backupId: string;
  at: string;
  operation: SwitchOperation;
}

/** Closed, renderer-safe runtime events emitted by the application backend. */
export type RuntimeLogAction =
  | "appStarted"
  | "appSettingsSaved"
  | "profileStoreReset"
  | "profileCreated"
  | "profileUpdated"
  | "profileDeleted"
  | "profilesReordered"
  | "profileImported"
  | "commonSettingsSaved"
  | "commonSettingsApplied"
  | "globalPromptDocumentSaved"
  | "configurationSwitched"
  | "backupRestored"
  | "switchUndone"
  | "staleLockRecovered"
  | "cloudBackupSettingsSaved"
  | "cloudBackupUploaded"
  | "cloudBackupRestored"
  | "sessionResumed"
  | "ccSwitchProfilesImported";

/** Persisted recording threshold. `silent` stops future event writes. */
export type RuntimeLogLevel = "debug" | "info" | "warn" | "error" | "silent";

/** Severity of one event already written to the application log. */
export type RuntimeLogSeverity = Exclude<RuntimeLogLevel, "silent">;

/** One non-secret application runtime event from the app-owned log files. */
export interface RuntimeLogEntry {
  at: string;
  level: RuntimeLogSeverity;
  action: RuntimeLogAction;
  errorCode?: string;
}

export type MatchStatus =
  | { kind: "matchesProfile"; profileId: string; profileName: string }
  | { kind: "profileChanged"; profileName: string }
  | { kind: "restoredBackup"; at: string }
  | { kind: "matchesSettings"; at: string }
  | { kind: "externallyModified"; at: string }
  | { kind: "unmanaged" }
  | { kind: "unknown" };

/** Reachability grade of one manual probe: any HTTP answer counts as ok/slow,
 * only network-level failures (DNS / refused / TLS / timeout) are unreachable. */
export type ProbeGrade = "ok" | "slow" | "unreachable";

export interface ProbeResult {
  grade: ProbeGrade;
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

export function getGlobalPromptDocument(app: AppKind): Promise<GlobalPromptDocument> {
  return invoke<GlobalPromptDocument>("get_global_prompt_document", { target: app });
}

export function saveGlobalPromptDocument(
  app: AppKind,
  content: string,
  expectedHash: string,
  confirmWrite: boolean,
): Promise<GlobalPromptDocument> {
  return invoke<GlobalPromptDocument>("save_global_prompt_document", {
    target: app,
    content,
    expectedHash,
    confirmWrite,
  });
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
  /** Font family for display and interface text; the value is quoted
   * verbatim as a CSS font-family, so it must be a plain family name. */
  interfaceFont: string;
  /** Threshold used for future application runtime-event recording. */
  runtimeLogLevel: RuntimeLogLevel;
}

/** Public connection coordinates for a user-owned Supabase project. The
 * Supabase account password and cloud-backup password are action-only inputs
 * and are never persisted. */
export interface CloudBackupSettings {
  projectUrl: string;
  publishableKey: string;
  email: string;
}

export interface CloudBackupResult {
  updatedAt: string;
  profileCount: number;
}

export function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_app_settings");
}

export function setAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("set_app_settings", { settings });
}

export function getCloudBackupSettings(): Promise<CloudBackupSettings | null> {
  return invoke<CloudBackupSettings | null>("get_cloud_backup_settings");
}

export function setCloudBackupSettings(
  settings: CloudBackupSettings,
): Promise<CloudBackupSettings> {
  return invoke<CloudBackupSettings>("set_cloud_backup_settings", { settings });
}

export function getCloudBackupSetupSql(): Promise<string> {
  return invoke<string>("cloud_backup_setup_sql");
}

export function uploadCloudBackup(
  accountPassword: string,
  backupPassword: string,
  confirmWrite: boolean,
): Promise<CloudBackupResult> {
  return invoke<CloudBackupResult>("upload_cloud_backup", {
    accountPassword,
    backupPassword,
    confirmWrite,
  });
}

export function restoreCloudBackup(
  accountPassword: string,
  backupPassword: string,
  confirmWrite: boolean,
): Promise<CloudBackupResult> {
  return invoke<CloudBackupResult>("restore_cloud_backup", {
    accountPassword,
    backupPassword,
    confirmWrite,
  });
}

/** Installed system font families, offered by the interface-font picker. */
export function listSystemFonts(): Promise<string[]> {
  return invoke<string[]>("list_system_fonts");
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

export function listRuntimeLogs(): Promise<RuntimeLogEntry[]> {
  return invoke<RuntimeLogEntry[]>("list_runtime_logs");
}

/** Opens the app-owned runtime-log directory without exposing its path to the UI. */
export function openRuntimeLogDir(): Promise<void> {
  return invoke<void>("open_runtime_log_dir");
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

/** Runs one on-demand usage-balance query with the supplied profile or editor
 * credential; nothing is persisted and the credential never appears in errors. */
export function testUsageQuery(
  query: UsageQuery,
  apiKey: string,
  baseUrl: string | null,
): Promise<UsageSummary> {
  return invoke<UsageSummary>("test_usage_query", { query, apiKey, baseUrl });
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

/** Public, global reset signals from Codex Runway. These do not describe the
 * signed-in account's actual quota or entitlement. */
export type CodexResetFeedStatus = "ok" | "degraded";

export interface ResetSignal {
  announcedAt: string;
  effectiveAt: string | null;
  schedulePrecision: string | null;
  confidence: number;
}

export interface TiboPost {
  announcedAt: string;
  text: string;
  url: string;
}

export interface CodexResetStatus {
  sourceUrl: string;
  feedStatus: CodexResetFeedStatus;
  generatedAt: string;
  lastSuccessfulCheckAt: string;
  checkedAt: string;
  latestConfirmedReset: ResetSignal | null;
  nextScheduledReset: ResetSignal | null;
  latestRelevantTiboPost: TiboPost | null;
  sourceWarning: string | null;
}

export type CodexResetFreshness = "cached" | "live";

/** The normalized public signal plus how the overview obtained it. */
export interface CodexResetRead {
  status: CodexResetStatus;
  freshness: CodexResetFreshness;
  cacheWarning: string | null;
}

/** Reads only the last successful local snapshot; it never contacts the feed. */
export function getCachedCodexResetStatus(): Promise<CodexResetRead | null> {
  return invoke<CodexResetRead | null>("get_cached_codex_reset_status");
}

export function checkCodexResetStatus(): Promise<CodexResetRead> {
  return invoke<CodexResetRead>("check_codex_reset_status");
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
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("window_minimize");
}

export function toggleMaximizeWindow(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("window_toggle_maximize");
}

export function closeWindow(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("window_close");
}

export function getWindowMaximized(): Promise<boolean> {
  if (isBrowserDevelopment) return Promise.resolve(false);
  return invoke<boolean>("window_is_maximized");
}

export function onWindowResized(handler: () => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return getCurrentWindow().onResized(() => handler());
}
