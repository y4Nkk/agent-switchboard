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
  /** Minutes between automatic re-queries of the expanded panel; 0 keeps
   * the panel manual-only. */
  refreshIntervalMinutes: number;
}

/** A self-authored JavaScript query. The source evaluates to `{ request,
 * extract }`; it is executed in the backend's restricted runtime. */
export interface ScriptUsageQuery {
  kind: "script";
  source: string;
  /** Minutes between automatic re-queries of the expanded panel; 0 keeps
   * the panel manual-only. */
  refreshIntervalMinutes: number;
}

/** The only persisted usage-query contract. `null` on a profile means the
 * optional feature is not configured. */
export type UsageQuery = DeclarativeUsageQuery | ScriptUsageQuery;

/** One named or unnamed usage reading. */
export interface UsageReading {
  planName?: string;
  remaining: number | null;
  used: number | null;
  total: number | null;
  unit: string | null;
}

/** Complete result of one usage-query response. */
export interface UsageSummary {
  readings: UsageReading[];
  at: string;
}

/** Renderer-safe state of the read-only Codex ChatGPT-login quota service. */
export type CodexOfficialQuotaStatus =
  | "available"
  | "signInRequired"
  | "reauthenticationRequired"
  | "unavailable";

export interface CodexOfficialQuotaWindow {
  label: string;
  usedPercent: number;
  resetsAt: string | null;
}

/** How a locally detected reset relates to the previously declared schedule. */
export type CodexOfficialQuotaResetKind = "scheduled" | "early";

/** One reset observed by comparing consecutive successful official reads. */
export interface CodexOfficialQuotaReset {
  observedAt: string;
  kind: CodexOfficialQuotaResetKind;
  resetsAt: string | null;
}

/** OAuth credentials and account identifiers never appear in this type. */
export interface CodexOfficialQuota {
  status: CodexOfficialQuotaStatus;
  windows: CodexOfficialQuotaWindow[];
  at: string | null;
  stale: boolean;
  lastReset: CodexOfficialQuotaReset | null;
}

export interface ProviderProfile {
  id: string;
  app: AppKind;
  routeMode: "official" | "custom";
  name: string;
  model: string | null;
  baseUrl: string | null;
  apiKey: string;
  modelOptions: ModelOptions | null;
  /** Local-only note; never written into any client configuration. */
  notes?: string | null;
  /** Provider homepage, used for navigation only. */
  websiteUrl: string | null;
  /** Application-side usage-balance query; never written into client config. */
  usageQuery?: UsageQuery | null;
}

/** One provider file together with its storage revision. `fileHash` guards
 * every mutation of that provider file against an external change. */
export interface ProviderRecord {
  profile: ProviderProfile;
  fileHash: string;
}

/** Routing is explicit. Official profiles retain no endpoint, API key, model
 * override, or usage script because client credentials stay native. */
export interface ProviderDraft {
  app: AppKind;
  routeMode: "official" | "custom";
  name: string;
  model: string | null;
  baseUrl: string | null;
  apiKey: string;
  modelOptions: ModelOptions | null;
  notes?: string | null;
  websiteUrl: string | null;
  usageQuery?: UsageQuery | null;
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

/** One concrete client configuration value. */
export type ConfigValue = boolean | string | number;

/** One application-owned setting intent. Automatic means that no line/key is
 * written to the client configuration; explicit values are always projected. */
export type CommonValue =
  | { mode: "automatic" }
  | { mode: "explicit"; value: ConfigValue };

/** The complete general-parameter values for one client, stored in the
 * application's `configuration/common/{client}.json`. */
export interface CommonSettings {
  settings: Record<string, CommonValue>;
}

export interface CommonChoiceOption {
  value: string;
  label: string;
}

/** One ownership-catalog general parameter the settings page may edit. */
export type CommonSettingSpec =
  | {
      key: string;
      label: string;
      group: string;
      control: "toggle";
      options: [];
    }
  | {
      key: string;
      label: string;
      group: string;
      control: "slider" | "segment";
      options: CommonChoiceOption[];
    };

/** One official configuration family with its real editing boundary. Paths
 * are labels from the typed backend directory, never renderer-supplied file
 * targets. */
export interface OfficialSettingDirectoryEntry {
  title: string;
  paths: string[];
  disposition: "direct" | "separateModule" | "preserveOnly";
  detail: string;
}

/** Full typed general-settings editing model. `settingsHash` is the
 * optimistic application-store revision; it is unrelated to client-file
 * hashes. */
export interface CommonSettingsEditor {
  app: AppKind;
  settings: CommonSettings;
  settingsHash: string;
  groups: string[];
  specs: CommonSettingSpec[];
  directory: OfficialSettingDirectoryEntry[];
}

export interface CommonSettingsSnapshot {
  settings: CommonSettings;
  settingsHash: string;
}

/** Read-only rendering of the current draft's shared settings only. */
export interface CommonSettingsPreview {
  app: AppKind;
  target: string;
  content: string;
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

export type WriteOperation = "projection" | "restore";

export interface ConfigWriteRecord {
  app: AppKind;
  profileId: string | null;
  profileName: string | null;
  contentHash: string;
  backupId: string;
  at: string;
  operation: WriteOperation;
}

/** Closed, renderer-safe runtime events emitted by the application backend. */
export type RuntimeLogAction =
  | "appStarted"
  | "appSettingsSaved"
  | "appSettingsRepaired"
  | "profileStoreReset"
  | "profileCreated"
  | "profileUpdated"
  | "profileDeleted"
  | "profilesReordered"
  | "profileImported"
  | "globalPromptDocumentSaved"
  | "configurationSwitched"
  | "backupRestored"
  | "switchUndone"
  | "staleLockRecovered"
  | "cloudBackupSettingsSaved"
  | "cloudBackupUploaded"
  | "cloudBackupRestored"
  | "sessionResumed"
  | "ccSwitchProfilesImported"
  | "officialLoginCompleted";

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
  lastSwitch: ConfigWriteRecord | null;
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
  routeMode: "official" | "custom";
  name: string;
  model: string | null;
  baseUrl: string | null;
  usageScriptImportable: boolean;
  usageScriptUpdatesExisting: boolean;
  warnings: string[];
  existing: boolean;
}

export interface CcSwitchScan {
  dbPath: string;
  providers: CcSwitchScanItem[];
  skipped: CcSwitchSkip[];
}

export interface CcSwitchImportOutcome {
  importedCount: number;
  usageScriptImportedCount: number;
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

export function listProfiles(): Promise<ProviderRecord[]> {
  return invoke<ProviderRecord[]>("list_profiles");
}

export function resetProfileStore(confirmWrite: boolean): Promise<void> {
  return invoke<void>("reset_profile_store", { confirmWrite });
}

export function createProfile(draft: ProviderDraft): Promise<ProviderRecord> {
  return invoke<ProviderRecord>("create_profile", { draft });
}

export function updateProfile(
  profileId: string,
  draft: ProviderDraft,
  expectedFileHash: string,
): Promise<ProviderRecord> {
  return invoke<ProviderRecord>("update_profile", { profileId, draft, expectedFileHash });
}

export function deleteProfile(profileId: string, expectedFileHash: string): Promise<void> {
  return invoke<void>("delete_profile", { profileId, expectedFileHash });
}

export function reorderProfiles(
  target: AppKind,
  orderedIds: string[],
  expectedFileHashes: Record<string, string>,
): Promise<ProviderRecord[]> {
  return invoke<ProviderRecord[]>("reorder_profiles", {
    target,
    orderedIds,
    expectedFileHashes,
  });
}

export function importDiscoveredProfile(target: AppKind): Promise<ProviderRecord> {
  return invoke<ProviderRecord>("import_discovered_profile", { target });
}

export function scanCcswitch(): Promise<CcSwitchScan> {
  return invoke<CcSwitchScan>("scan_ccswitch");
}

export function importCcswitchProfiles(keys: string[]): Promise<CcSwitchImportOutcome> {
  return invoke<CcSwitchImportOutcome>("import_ccswitch_profiles", { keys });
}

/** Reads the stored general-parameter values plus the catalog that can edit
 * them. This does not read a real Codex or Claude Code configuration file. */
export function getCommonSettingsEditor(app: AppKind): Promise<CommonSettingsEditor> {
  return invoke<CommonSettingsEditor>("get_common_settings_editor", { target: app });
}

/** Saves desired application state only. A supplier must subsequently be
 * re-applied through the normal switch flow to project it into a client file. */
export function saveCommonSettings(
  app: AppKind,
  settings: CommonSettings,
  expectedSettingsHash: string,
): Promise<CommonSettingsSnapshot> {
  return invoke<CommonSettingsSnapshot>("save_common_settings", {
    target: app,
    settings,
    expectedSettingsHash,
  });
}

/** Renders the current common-settings draft without reading or writing a
 * real client file. */
export function previewCommonSettings(
  app: AppKind,
  settings: CommonSettings,
): Promise<CommonSettingsPreview> {
  return invoke<CommonSettingsPreview>("preview_common_settings", {
    target: app,
    settings,
  });
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
  launchAtLogin: boolean;
  hardwareAcceleration: boolean;
  /** Font family for display and interface text; the value is quoted
   * verbatim as a CSS font-family, so it must be a plain family name. */
  interfaceFont: string;
  /** Threshold used for future application runtime-event recording. */
  runtimeLogLevel: RuntimeLogLevel;
  /** Provider ids whose usage panel is collapsed; any other provider's
   * panel is expanded. */
  collapsedUsageIds: string[];
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

/** Replaces an invalid settings file with defaults; a readable file refuses. */
export function repairAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("repair_app_settings");
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

/** One model from the provider's OpenAI-compatible /v1/models endpoint; the
 * optional vendor groups the editor's model picker. */
export interface ProviderModel {
  id: string;
  ownedBy: string | null;
}

/** Models from the provider's OpenAI-compatible /v1/models endpoint. */
export function fetchProviderModels(baseUrl: string, apiKey: string): Promise<ProviderModel[]> {
  return invoke<ProviderModel[]>("fetch_provider_models", { url: baseUrl, apiKey });
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

/** Runs a persisted provider query without exposing its credential to the UI.
 * Successful summaries are retained by the desktop runtime for tray display. */
export function queryProfileUsage(profileId: string): Promise<UsageSummary> {
  return invoke<UsageSummary>("query_profile_usage", { profileId });
}

/** Reads the current native Codex official-login quota without accepting any
 * renderer credential, endpoint, or raw OAuth account data. */
export function queryCodexOfficialQuota(profileId: string): Promise<CodexOfficialQuota> {
  return invoke<CodexOfficialQuota>("query_codex_official_quota", { profileId });
}

/** Reads the persisted last successful official read without contacting the
 * network. Absent until the first refresh of the machine's Codex login. */
export function getCachedCodexOfficialReset(): Promise<CodexOfficialQuota | null> {
  return invoke<CodexOfficialQuota | null>("get_cached_codex_official_reset");
}

/** One explicit read of the machine's Codex official login. Account-scoped
 * and profile-independent; failed reads return as statuses, not errors. */
export function refreshCodexOfficialReset(): Promise<CodexOfficialQuota> {
  return invoke<CodexOfficialQuota>("refresh_codex_official_reset");
}

/** Phases of one in-flight official login. */
export type OfficialLoginPhase = "pending" | "completed" | "failed";

/** Start payload: the device code to enter (Codex) or the authorize URL to
 * open (Claude). Never carries a credential. */
export interface OfficialLoginStart {
  userCode: string | null;
  verificationUrl: string;
}

/** Poll result: only status, codes, URLs, and fixed messages — no tokens. */
export interface OfficialLoginStatus {
  phase: OfficialLoginPhase;
  userCode: string | null;
  verificationUrl: string;
  message: string | null;
}

/** Starts the official login flow for one client; the previous session for
 * that client must be finished or cancelled first. */
export function startOfficialLogin(target: AppKind): Promise<OfficialLoginStart> {
  return invoke<OfficialLoginStart>("official_login_start", { target });
}

/** Advances one login by a single step and writes the client's native
 * credential cache once the vendor approves. */
export function pollOfficialLogin(target: AppKind): Promise<OfficialLoginStatus> {
  return invoke<OfficialLoginStatus>("official_login_poll", { target });
}

/** Cancels one in-flight official login; a no-op without a running session. */
export function cancelOfficialLogin(target: AppKind): Promise<void> {
  return invoke<void>("official_login_cancel", { target });
}

/** Result of one startup or user-triggered app-update check; informational only. */
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

/** The previous successful scan, cached by the backend; null before the first
 * scan ever completed. */
export function discoverCached(): Promise<DiscoveryReport | null> {
  return invoke<DiscoveryReport | null>("discover_cached");
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

/** Restarts the native desktop process so creation-time WebView settings apply. */
export function restartApplication(): Promise<void> {
  if (isBrowserDevelopment) return Promise.resolve();
  return invoke("restart_application");
}

export function getWindowMaximized(): Promise<boolean> {
  if (isBrowserDevelopment) return Promise.resolve(false);
  return invoke<boolean>("window_is_maximized");
}

export function onWindowResized(handler: () => void): Promise<() => void> {
  if (isBrowserDevelopment) return Promise.resolve(() => {});
  return getCurrentWindow().onResized(() => handler());
}
