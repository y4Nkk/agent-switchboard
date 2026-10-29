import type { LocalizedMessage } from "./switching";
import { invoke } from "./client";
import type { AppKind, UpstreamProtocol } from "./shared";
import type { SettingsValues } from "./settings";
import type { RouteState, ConfigWriteRecord } from "./switching";

/** Closed, renderer-safe runtime events emitted by the application backend. */
export type RuntimeLogAction =
  | "appStarted"
  | "storageCacheMaintained"
  | "storageHistoryArchived"
  | "storageBackupsCleaned"
  | "storageSessionsCleaned"
  | "updateTempSwept"
  | "appSettingsSaved"
  | "outboundProxySaved"
  | "outboundProxyRepaired"
  | "codexSubagentSettingsApplied"
  | "providersSqlImported"
  | "appSettingsRepaired"
  | "profileStoreRepaired"
  | "applicationDataRepaired"
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
  | "cloudBackupConnectionTested"
  | "cloudBackupUploaded"
  | "cloudBackupRestored"
  | "sessionResumed"
  | "sessionDeleted"
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

export interface ConfigFileStatus {
  app: AppKind;
  path: string;
  exists: boolean;
  syntaxOk: boolean;
  route: RouteState | null;
  readError: string | null;
  /** Derived from pending configuration transactions; no separate persisted status. */
  recoveryIssue: string | null;
  /** Values read from the real client configuration, never from ASB storage. */
  clientSettings: SettingsValues | null;
  clientSettingsError: string | null;
  /** Connection identity, independent of model, preferences and file health. */
  activeProfileId: string | null;
  lastSwitch: ConfigWriteRecord | null;
}

/** Read-only application metadata for the overview footer. It never includes
 * client configuration, credentials, or account information. */
export interface RuntimeOverview {
  appVersion: string;
  buildMode: "debug" | "release";
  platform: string;
  architecture: string;
  transport: RuntimeTransport;
  appDataPath: string;
}

export type RuntimeTransport =
  | { kind: "desktopProtocol" }
  | { kind: "webDevelopment"; host: string; port: number; healthStatus: number };

/** One active loopback route on the gateway status page. The page resolves
 * the display name from its own profile snapshot via profileId. */
export interface GatewayRouteStatus {
  app: AppKind;
  profileId: string;
  upstreamProtocol: UpstreamProtocol;
}

/** One completed gateway request. `status` is the HTTP status, or null when
 * a WebSocket exchange ended without a complete converted answer.
 * `profileId`, `routeRevision`, and `upstreamProtocol` are set only when the
 * request matched an active route; rejected traffic stays unattributed. */
export interface GatewaySample {
  atMs: number;
  app: AppKind;
  profileId: string | null;
  routeRevision: string | null;
  clientProtocol: UpstreamProtocol;
  upstreamProtocol: UpstreamProtocol | null;
  status: number | null;
  durationMs: number;
  requestBytes: number;
  responseBytes: number;
}

export interface GatewayMetricsStatus {
  startedAtMs: number;
  totalRequests: number;
  failedRequests: number;
  samples: GatewaySample[];
}

/** The listener's runtime condition. A saved port is never presented as a
 * listening one: `standby`/`running` imply `listeningPort` is set, while the
 * failure states imply it is null. */
export type GatewayStatusKind =
  | "standby"
  | "running"
  | "portConflict"
  | "bindRejected"
  | "needsRepair"
  | "recoveryBlocked";

/** The process holding the configured port, when the platform could identify
 * it reliably; an unknown holder stays null and is never terminated. */
export interface GatewayPortProcessStatus {
  pid: number;
  name: string | null;
}

export type GatewayFailureKind = "portInUse" | "systemRejected" | "stateUnusable";

/** Why the listener is down: the port, the OS error code, the identified
 * holder process when known, and a scrubbed user-readable message. */
export interface GatewayFailureStatus {
  port: number;
  kind: GatewayFailureKind;
  osCode: number | null;
  message: string;
  process: GatewayPortProcessStatus | null;
}

/** A port-change transaction stuck before its commit point because a
 * rollback target was modified externally. Kept until explicitly resolved. */
export interface GatewayBlockedRecoveryStatus {
  fromPort: number;
  toPort: number;
  apps: AppKind[];
  reason: string;
}

export interface GatewayStatus {
  /** The saved port that client configurations are built against. */
  configuredPort: number;
  /** The port actually being served, or null while not listening. */
  listeningPort: number | null;
  /** The served loopback base URL, or null while not listening. */
  baseUrl: string | null;
  status: GatewayStatusKind;
  failure: GatewayFailureStatus | null;
  repairReason: string | null;
  blockedRecovery: GatewayBlockedRecoveryStatus | null;
  routes: GatewayRouteStatus[];
  metrics: GatewayMetricsStatus;
}

/** One client configuration the confirmed port change will rewrite. */
export interface GatewayPortChangeClientPlan {
  app: AppKind;
  profileId: string;
  profileName: string;
  currentBaseUrl: string;
  newBaseUrl: string;
}

/** The one-shot plan bound to a held socket, produced by the preparation. */
export interface GatewayPortChangePlan {
  preparationId: string;
  fromPort: number;
  toPort: number;
  clients: GatewayPortChangeClientPlan[];
}

export interface GatewayPortChangeResult {
  fromPort: number;
  toPort: number;
  clients: GatewayPortChangeClientPlan[];
  warnings: LocalizedMessage[];
}

export function getConfigStatus(): Promise<ConfigFileStatus[]> {
  return invoke<ConfigFileStatus[]>("config_status");
}

/** Reveals the client's real configuration file in its folder. The backend
 * resolves the path; the UI never sends one. */
export function openConfigFileLocation(app: AppKind): Promise<void> {
  return invoke<void>("open_config_file_location", { target: app });
}

/** Opens the application data directory reported by the runtime overview. */
export function openAppDataDir(): Promise<void> {
  return invoke<void>("open_app_data_dir");
}

export function getRuntimeOverview(): Promise<RuntimeOverview> {
  return invoke<RuntimeOverview>("runtime_overview");
}

export function getGatewayStatus(): Promise<GatewayStatus> {
  return invoke<GatewayStatus>("gateway_status");
}

/** Retries binding the configured port after a failed start. */
export function retryGatewayBind(): Promise<GatewayStatus> {
  return invoke<GatewayStatus>("gateway_retry_bind");
}

/** Validates the new port, holds its socket, and identifies the client
 * configurations to rewrite. No file is modified. */
export function prepareGatewayPortChange(newPort: number): Promise<GatewayPortChangePlan> {
  return invoke<GatewayPortChangePlan>("gateway_prepare_port_change", { newPort });
}

/** Applies the confirmed port change as one recoverable transaction. */
export function commitGatewayPortChange(
  preparationId: string,
  confirmWrite: boolean,
): Promise<GatewayPortChangeResult> {
  return invoke<GatewayPortChangeResult>("gateway_commit_port_change", {
    preparationId,
    confirmWrite,
  });
}

/** Releases the one-shot prepared socket without changing configuration. */
export function cancelGatewayPortChange(preparationId: string): Promise<void> {
  return invoke<void>("gateway_cancel_port_change", { preparationId });
}

/** Discards a blocked port-change recovery, keeping the current externally
 * modified configuration. */
export function discardGatewayPortChange(confirmWrite: boolean): Promise<GatewayStatus> {
  return invoke<GatewayStatus>("gateway_discard_port_change", { confirmWrite });
}

export interface RuntimeLogRead {
  entries: RuntimeLogEntry[];
  total: number;
  skipped: number;
}

export function listRuntimeLogs(level: RuntimeLogSeverity | null): Promise<RuntimeLogRead> {
  return invoke<RuntimeLogRead>("list_runtime_logs", { level });
}

/** Opens the app-owned runtime-log directory without exposing its path to the UI. */
export function openRuntimeLogDir(): Promise<void> {
  return invoke<void>("open_runtime_log_dir");
}
