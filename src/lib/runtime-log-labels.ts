import type { MessageKey } from "../i18n/messages.ts";

export const RUNTIME_LOG_ACTION_LABELS = {
  appStarted: "backup.action.appStarted",
  webviewMemoryLow: "backup.action.webviewMemoryLow",
  webviewMemoryNormal: "backup.action.webviewMemoryNormal",
  storageCacheMaintained: "backup.action.storageCacheMaintained",
  storageSessionIndexCleared: "backup.action.storageSessionIndexCleared",
  storageHistoryArchived: "backup.action.storageHistoryArchived",
  storageBackupsCleaned: "backup.action.storageBackupsCleaned",
  storageSessionsCleaned: "backup.action.storageSessionsCleaned",
  updateTempSwept: "backup.action.updateTempSwept",
  appSettingsSaved: "backup.action.appSettingsSaved",
  outboundProxySaved: "backup.action.outboundProxySaved",
  codexSubagentSettingsApplied: "backup.action.codexSubagentSettingsApplied",
  providersSqlImported: "backup.action.providersSqlImported",
  applicationDataRepaired: "backup.action.applicationDataRepaired",
  configurationReadFailed: "backup.action.configurationReadFailed",
  profileCreated: "backup.action.profileCreated",
  profileUpdated: "backup.action.profileUpdated",
  profileDeleted: "backup.action.profileDeleted",
  profilesReordered: "backup.action.profilesReordered",
  profileImported: "backup.action.profileImported",
  globalPromptDocumentSaved: "backup.action.globalPromptDocumentSaved",
  configurationSwitched: "backup.action.configurationSwitched",
  backupRestored: "backup.action.backupRestored",
  switchUndone: "backup.action.switchUndone",
  staleLockRecovered: "backup.action.staleLockRecovered",
  cloudBackupSettingsSaved: "backup.action.cloudBackupSettingsSaved",
  cloudBackupConnectionTested: "backup.action.cloudBackupConnectionTested",
  cloudBackupUploaded: "backup.action.cloudBackupUploaded",
  cloudBackupRestored: "backup.action.cloudBackupRestored",
  sessionResumed: "backup.action.sessionResumed",
  sessionDeleted: "backup.action.sessionDeleted",
  sessionRangeProcessed: "backup.action.sessionRangeProcessed",
  codexSessionProvidersTransferred: "backup.action.codexSessionProvidersTransferred",
  codexSessionProvidersRestored: "backup.action.codexSessionProvidersRestored",
  ccSwitchProfilesImported: "backup.action.ccSwitchProfilesImported",
  officialLoginCompleted: "backup.action.officialLoginCompleted",
} as const satisfies Record<string, MessageKey>;

export const RUNTIME_LOG_LEVEL_LABELS = {
  debug: "backup.level.debug",
  info: "backup.level.info",
  warn: "backup.level.warn",
  error: "backup.level.error",
  silent: "backup.level.silent",
} as const satisfies Record<string, MessageKey>;

export type RuntimeLogAction = keyof typeof RUNTIME_LOG_ACTION_LABELS;
export type RuntimeLogLevel = keyof typeof RUNTIME_LOG_LEVEL_LABELS;
export type RuntimeLogSeverity = Exclude<RuntimeLogLevel, "silent">;
export type RuntimeLogFilter = "all" | RuntimeLogSeverity;

export const LOG_LEVEL_OPTIONS = (Object.keys(RUNTIME_LOG_LEVEL_LABELS) as RuntimeLogLevel[])
  .map((value) => ({ value, labelKey: RUNTIME_LOG_LEVEL_LABELS[value] }));

export const LEVEL_FILTERS: ReadonlyArray<{ value: RuntimeLogFilter; labelKey: MessageKey }> = [
  { value: "all", labelKey: "backup.level.all" },
  ...LOG_LEVEL_OPTIONS.flatMap(({ value, labelKey }) => value === "silent" ? [] : [{ value, labelKey }]),
];
