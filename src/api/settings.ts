import { invoke } from "./client";
import type { AppKind, CommandError } from "./shared";
import type { RuntimeLogLevel } from "./status";
import type { CodexSubagentSettings } from "./subagent-settings";

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
export type ConfigValue = boolean | string | number | string[];

/** One application-owned setting intent. Automatic means that no line/key is
 * written to the client configuration; explicit values are always projected. */
export type SettingValue =
  | { mode: "automatic" }
  | { mode: "explicit"; value: ConfigValue };

/** Complete values for one backend-owned settings catalog. */
export interface SettingsValues {
  /** Claude-only supplemental fields; visual preferences keep their existing owner. */
  claudeExtra?: Record<string, unknown>;
  settings: Record<string, SettingValue>;
}

export interface SettingChoiceOption {
  value: string;
  label: string;
}

/** One backend-owned parameter or client preference. */
export type SettingSpec =
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
      options: SettingChoiceOption[];
    }
  | {
      key: string;
      label: string;
      group: string;
      control: "text" | "notifications";
      options: [];
    }
  | {
      key: string;
      label: string;
      group: string;
      control: "integer" | "stringInteger";
      min: number;
      max: number;
      options: [];
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

/** Full typed client-preference editing model. `settingsHash` is the
 * optimistic application-store revision; it is unrelated to client-file
 * hashes. */
export interface ClientSettingsEditor {
  app: AppKind;
  settings: SettingsValues;
  settingsHash: string;
  groups: string[];
  specs: SettingSpec[];
  directory: OfficialSettingDirectoryEntry[];
}

export interface ClientSettingsSnapshot {
  settings: SettingsValues;
  settingsHash: string;
}

/** One hash-bound client-configuration file candidate. */
export interface ClientConfigurationApplyPreview {
  file: import("./switching").FilePreview;
  settingsHash: string;
  targetExisted: boolean;
}

/** A backend-owned client-configuration reset scope. */
export type ClientConfigurationResetKind =
  | "nativeDefaults"
  | "nativeDefaultsWithUnmanaged"
  | "clearExtraConfiguration";

/** The two native-defaults reset scopes (standard and advanced); the
 * Claude-only extra-configuration clear is a separate operation. */
export type NativeConfigurationResetKind = Exclude<ClientConfigurationResetKind, "clearExtraConfiguration">;

export interface ProviderParametersCatalog {
  app: AppKind;
  defaults: SettingsValues;
  groups: string[];
  specs: SettingSpec[];
  claudeEffortOptions: SettingChoiceOption[];
}

export function getProviderParametersCatalog(app: AppKind): Promise<ProviderParametersCatalog> {
  return invoke<ProviderParametersCatalog>("get_provider_parameters_catalog", { target: app });
}

/** A redacted snapshot of the actual client configuration file. */
export interface CurrentClientConfiguration {
  app: AppKind;
  target: string;
  exists: boolean;
  content: string;
  contentHash: string;
  syntaxOk: boolean;
  syntaxError: string | null;
}

/** Reads the stored client-preference values plus the catalog that can edit
 * them. This does not read a real Codex or Claude Code configuration file. */
export function getClientSettingsEditor(app: AppKind): Promise<ClientSettingsEditor> {
  return invoke<ClientSettingsEditor>("get_client_settings_editor", { target: app });
}

/** Saves desired application state only. A supplier must subsequently be
 * re-applied through the normal switch flow to project it into a client file. */
export function saveClientSettings(
  app: AppKind,
  settings: SettingsValues,
  expectedSettingsHash: string,
): Promise<ClientSettingsSnapshot> {
  return invoke<ClientSettingsSnapshot>("save_client_settings", {
    target: app,
    settings,
    expectedSettingsHash,
  });
}

/** Reads a redacted copy of the actual local client configuration. Manual
 * edits retain redacted source values only through the confirmed backend executor. */
export function getCurrentClientConfiguration(app: AppKind): Promise<CurrentClientConfiguration> {
  return invoke<CurrentClientConfiguration>("get_current_client_configuration", { target: app });
}

export function previewClientConfigurationApply(
  app: AppKind,
  settings: SettingsValues,
  subagentSettings?: CodexSubagentSettings,
): Promise<ClientConfigurationApplyPreview> {
  return invoke<ClientConfigurationApplyPreview>("preview_client_configuration_apply", {
    target: app, settings, subagentSettings,
  });
}

/** Previews one backend-owned reset without accepting configuration values from the UI. */
export function previewClientConfigurationReset(
  app: AppKind,
  resetKind: ClientConfigurationResetKind,
): Promise<ClientConfigurationApplyPreview> {
  return invoke<ClientConfigurationApplyPreview>("preview_client_configuration_reset", {
    target: app, resetKind,
  });
}

export interface ClientConfigurationRepairPreview {
  file: import("./switching").FilePreview;
  targetExisted: boolean;
}

export function previewClientConfigurationRepair(
  app: AppKind,
  expectedSourceHash: string,
): Promise<ClientConfigurationRepairPreview> {
  return invoke<ClientConfigurationRepairPreview>("preview_client_configuration_repair", {
    target: app, expectedSourceHash,
  });
}

export function commitClientConfigurationRepair(
  app: AppKind,
  expectedSourceHash: string,
  expectedRenderedHash: string,
  expectedTargetExisted: boolean,
): Promise<void> {
  return invoke<void>("commit_client_configuration_repair", {
    target: app,
    expectedSourceHash,
    expectedRenderedHash,
    expectedTargetExisted,
    confirmWrite: true,
  });
}

export function previewManualClientConfiguration(
  app: AppKind,
  expectedSourceHash: string,
  displayContent: string,
  settings: SettingsValues,
  subagentSettings?: CodexSubagentSettings,
): Promise<ClientConfigurationApplyPreview> {
  return invoke<ClientConfigurationApplyPreview>("preview_manual_client_configuration", {
    target: app, expectedSourceHash, displayContent, settings, subagentSettings,
  });
}

export function commitManualClientConfiguration(
  app: AppKind,
  expectedSourceHash: string,
  expectedRenderedHash: string,
  expectedSettingsHash: string,
  expectedTargetExisted: boolean,
  displayContent: string,
  settings: SettingsValues,
  subagentSettings?: CodexSubagentSettings,
): Promise<void> {
  return invoke<void>("commit_manual_client_configuration", {
    target: app,
    expectedSourceHash,
    expectedRenderedHash,
    expectedSettingsHash,
    expectedTargetExisted,
    displayContent,
    settings,
    subagentSettings,
    confirmWrite: true,
  });
}

export function commitClientConfigurationApply(
  app: AppKind,
  settings: SettingsValues,
  preview: ClientConfigurationApplyPreview,
  subagentSettings?: CodexSubagentSettings,
): Promise<void> {
  return invoke<void>("commit_client_configuration_apply", {
    target: app,
    expectedHash: preview.file.contentHash,
    expectedRenderedHash: preview.file.renderedHash,
    expectedSettingsHash: preview.settingsHash,
    expectedTargetExisted: preview.targetExisted,
    settings,
    subagentSettings,
    confirmWrite: true,
  });
}

/** Commits the exact reset candidate that was previously previewed. */
export function commitClientConfigurationReset(
  app: AppKind,
  resetKind: ClientConfigurationResetKind,
  preview: ClientConfigurationApplyPreview,
): Promise<void> {
  return invoke<void>("commit_client_configuration_reset", {
    target: app,
    resetKind,
    expectedHash: preview.file.contentHash,
    expectedRenderedHash: preview.file.renderedHash,
    expectedSettingsHash: preview.settingsHash,
    expectedTargetExisted: preview.targetExisted,
    confirmWrite: true,
  });
}

/** Parses the dedicated Claude extra-configuration editor. */
export function parseClaudeExtraConfiguration(content: string): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("parse_claude_extra_configuration", { content });
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
/** Interface language. "system" resolves against the desktop locale at render
 * time; the explicit values are the BCP-47 tags both webviews resolve. */
export type LanguagePreference = "system" | "zh-CN" | "en-US";
export type InterfaceScale = 90 | 100 | 110 | 125;
export type StartupPage = "providers" | "lastVisited";
export type WorkspacePage = "providers" | "clientConfiguration" | "extensions" | "sessions" | "usage" | "settings";

/** Application-runtime desktop preferences; separate from client config. */
export interface AppSettings {
  closeBehavior: CloseBehavior;
  theme: ThemePreference;
  motion: MotionPreference;
  alwaysOnTop: boolean;
  launchAtLogin: boolean;
  /** Keeps the main window hidden at startup; the app starts in the tray. */
  startMinimized: boolean;
  hardwareAcceleration: boolean;
  /** Font family for display and interface text; the value is quoted
   * verbatim as a CSS font-family, so it must be a plain family name. */
  interfaceFont: string;
  interfaceScale: InterfaceScale;
  /** Empty disables the shortcut; otherwise stores one canonical physical-key chord. */
  globalShortcut: string;
  startupPage: StartupPage;
  /** Threshold used for future application runtime-event recording. */
  runtimeLogLevel: RuntimeLogLevel;
  /** Provider ids whose usage details are explicitly expanded. */
  expandedUsageIds: string[];
  /** Interface language for the main window and tray webviews. */
  language: LanguagePreference;
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

export interface AppSettingsSnapshot {
  settings: AppSettings;
  desktopError: CommandError | null;
}

export function getAppSettings(): Promise<AppSettingsSnapshot> {
  return invoke<AppSettingsSnapshot>("get_app_settings");
}

export function getStartupPage(): Promise<WorkspacePage> {
  return invoke<WorkspacePage>("get_startup_page");
}

export function rememberWorkspacePage(page: WorkspacePage): Promise<void> {
  return invoke("remember_workspace_page", { page });
}

export function getSidebarCollapsed(): Promise<boolean> {
  return invoke<boolean>("get_sidebar_collapsed");
}

export function setSidebarCollapsed(collapsed: boolean): Promise<boolean> {
  return invoke<boolean>("set_sidebar_collapsed", { collapsed });
}

export function setShortcutRecording(recording: boolean): Promise<void> {
  return invoke("set_shortcut_recording", { recording });
}

export function setAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("set_app_settings", { settings });
}

/** Replaces an invalid settings file with defaults; a readable file refuses. */
export function repairAppSettings(): Promise<AppSettingsSnapshot> {
  return invoke<AppSettingsSnapshot>("repair_app_settings");
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

/** Verifies an unsaved cloud-backup draft without writing remote data. */
export function testCloudBackupConnection(
  settings: CloudBackupSettings,
  accountPassword: string,
): Promise<void> {
  return invoke<void>("test_cloud_backup_connection", { settings, accountPassword });
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
