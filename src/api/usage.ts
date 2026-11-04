import { invoke } from "./client";
import type { AppKind, CodexTransportClass, UpstreamProtocol } from "./shared";
import type { ProviderConnectionOptions } from "./providers";
import type { UsageReading } from "./usage-reading";
export type { UsageReading, UsageAvailability } from "./usage-reading";

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
  /** Minutes between the desktop scheduler's automatic re-queries of this
   * profile; 0 keeps it manual-only. */
  refreshIntervalMinutes: number;
}

/** A self-authored JavaScript query. The source evaluates to `{ request,
 * extract }`; the backend normalizes extracted facts into availability states. */
export interface ScriptUsageQuery {
  kind: "script";
  source: string;
  /** Minutes between the desktop scheduler's automatic re-queries of this
   * profile; 0 keeps it manual-only. */
  refreshIntervalMinutes: number;
}

/** The only persisted usage-query contract. `null` on a profile means the
 * optional feature is not configured. */
export type UsageQuery = DeclarativeUsageQuery | ScriptUsageQuery;

/** Complete result of one usage-query response. */
export interface UsageSummary {
  readings: UsageReading[];
  at: string;
}

export interface UsageSnapshot {
  summary: UsageSummary | null;
  error: string | null;
}

/** The renderer asks for either its current provider's normalized history or
 * the independent account-level Codex official history. */
export type UsageHistoryRequest =
  | { kind: "provider"; profileId: string }
  | { kind: "official" };

export type UsageHistoryMetric = "remaining" | "used" | "usedPercent";

export interface UsageHistoryPoint {
  at: string;
  value: number;
}

/** A credential-free, unit-aware historical series returned by the desktop
 * service after successful real reads only. */
export interface UsageHistorySeries {
  totalPoints: number;
  firstAt: string;
  lastAt: string;
  id: string;
  label: string;
  unit: string | null;
  metric: UsageHistoryMetric;
  points: UsageHistoryPoint[];
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
  /** Present exactly on unavailable and reauthenticationRequired reads. */
  failure: CodexQuotaReadFailure | null;
}

/** Why one official-quota read failed after an attempt was made. */
export interface CodexQuotaReadFailure {
  kind: "transport" | "httpStatus" | "invalidResponse";
  transportClass: CodexTransportClass | null;
  httpStatus: number | null;
}

/** Runs one on-demand usage-balance query with the supplied profile or editor
 * credential; nothing is persisted and the credential never appears in errors. */
export function testUsageQuery(
  query: UsageQuery,
  apiKey: string,
  baseUrl: string | null,
  upstreamProtocol: UpstreamProtocol,
  authentication?: import("./shared").AuthenticationScheme | null,
  connection?: ProviderConnectionOptions | null,
): Promise<UsageSummary> {
  return invoke<UsageSummary>("test_usage_query", {
    request: {
      query,
      apiKey,
      baseUrl,
      upstreamProtocol,
      ...(authentication ? { authentication } : {}),
      ...(connection ? { connection } : {}),
    },
  });
}

/** Runs one persisted provider query right now without exposing its
 * credential to the UI. Successful summaries are retained by the desktop
 * runtime for tray display. This is the forced manual path (the refresh
 * button); the backend scheduler owns automatic re-queries. */
export function queryProfileUsage(target: AppKind, profileId: string): Promise<void> {
  return invoke<void>("query_profile_usage", { target, profileId });
}

/** Reads the last completed query and any retained readings.
 * Null means no attempt has completed; this never contacts upstream. */
export function readProfileUsage(target: AppKind, profileId: string): Promise<UsageSnapshot | null> {
  return invoke<UsageSnapshot | null>("read_profile_usage", { target, profileId });
}

/** Refreshes the official profile's account-bound cache without accepting
 * renderer credentials, endpoints, or raw OAuth account data. */
export function queryCodexOfficialQuota(profileId: string): Promise<void> {
  return invoke<void>("query_codex_official_quota", { profileId });
}

/** Shares the account-checked cache projection used by the tray. */
export function readCodexOfficialQuota(profileId: string): Promise<CodexOfficialQuota | null> {
  return invoke<CodexOfficialQuota | null>("read_codex_official_quota", { profileId });
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

/** Public, global reset signals from Codex Runway. These do not describe the
 * signed-in account's actual quota or entitlement. */
export type CodexResetFeedStatus = "ok" | "degraded";
export type ResetType = "global" | "banked" | "other";

export interface ResetSignal {
  announcedAt: string;
  effectiveAt: string | null;
  schedulePrecision: string | null;
  confidence: number;
  resetType: ResetType;
}

export interface TiboPost {
  announcedAt: string;
  text: string;
  url: string;
}

/** The public feed's coarse per-UTC-day reset history (levels 0–4); days
 * without an entry had no signal. */
export interface CodexResetHeatmap {
  timezone: string;
  weeks: number;
  total: number;
  days: CodexResetHeatmapDay[];
}

export interface CodexResetHeatmapDay {
  date: string;
  count: number;
  level: number;
}

export interface CodexResetStatus {
  sourceUrl: string;
  feedStatus: CodexResetFeedStatus;
  generatedAt: string;
  lastSuccessfulCheckAt: string;
  checkedAt: string;
  latestConfirmedSignal: ResetSignal | null;
  nextScheduledReset: ResetSignal | null;
  latestRelevantTiboPost: TiboPost | null;
  heatmap: CodexResetHeatmap;
  sourceWarning: string | null;
}

export type CodexResetFreshness = "cached" | "live";

/** One source's normalized public signal plus how the overview obtained it.
 * Generic over the per-source status so both monitors share one envelope. */
export interface CodexResetRead<S = CodexResetStatus> {
  status: S;
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

/** Public, global reset signals from codex-reset.com, the second independent
 * monitor. These do not describe the signed-in account's quota or entitlement. */
export type HubConfidence = "low" | "medium" | "high";
export type HubEventKind = "reset" | "credits";
export type HubSurfaceStatus = "operational" | "degraded" | "outage" | "maintenance";

export interface HubChance {
  percent: number;
  rangeLower: number;
  rangeUpper: number;
}

export interface HubWindow {
  startHour: number;
  endHour: number;
  timezone: string;
}

export interface HubCadence {
  medianDays: number;
  sample: number;
  meanDays: number;
  accelerating: boolean;
}

export interface HubWait {
  days: number;
  sample: number;
  shorterShare: number;
  medianDays: number;
}

export interface HubForecast {
  updatedAt: string;
  confidence: HubConfidence;
  chance24h: HubChance;
  chance48h: HubChance;
  lastResetAt: string;
  ageDays: number;
  window: HubWindow | null;
  cadence: HubCadence | null;
  wait: HubWait | null;
  /** The site's own Simplified Chinese explanation of the current context. */
  contextNote: string | null;
}

export interface HubEvent {
  id: string;
  date: string;
  kind: HubEventKind;
  summary: string;
  url: string | null;
  announcedAt: string;
  confirmed: boolean;
}

export interface HubTimeline {
  updatedAt: string;
  resets: number;
  credits: number;
  events: HubEvent[];
}

export interface HubSurface {
  id: string;
  label: string;
  status: HubSurfaceStatus;
}

export interface HubPlatformStats {
  days: number;
  incidents: number;
  resolved: number;
  open: number;
  medianRecoveryMinutes: number | null;
}

export interface HubIncident {
  name: string;
  impact: string;
  resolved: boolean;
  startedAt: string;
  resolvedAt: string | null;
  url: string | null;
}

export interface HubPlatform {
  checkedAt: string;
  stale: boolean;
  surfaces: HubSurface[];
  stats: HubPlatformStats;
  incidents: HubIncident[];
}

export interface HubSignalPost {
  summary: string;
  at: string;
  url: string;
  active: boolean;
}

export interface HubBankedNote {
  at: string;
  text: string;
}

export interface HubFeed {
  fetchedAt: string;
  stale: boolean;
  signal: HubSignalPost | null;
  banked: HubBankedNote[];
}

export interface CodexResetHubStatus {
  sourceUrl: string;
  generatedAt: string;
  checkedAt: string;
  sourceWarning: string | null;
  forecast: HubForecast;
  timeline: HubTimeline;
  platform: HubPlatform;
  feed: HubFeed;
}

export type CodexResetHubRead = CodexResetRead<CodexResetHubStatus>;

/** Reads only the last successful local aggregate; it never contacts the source. */
export function getCachedCodexResetHubStatus(): Promise<CodexResetHubRead | null> {
  return invoke<CodexResetHubRead | null>("get_cached_codex_reset_hub_status");
}

export function checkCodexResetHubStatus(): Promise<CodexResetHubRead> {
  return invoke<CodexResetHubRead>("check_codex_reset_hub_status");
}

/** Reads the app-owned, credential-free history ledger. It never triggers a
 * provider or Codex network request. */
export function getUsageHistory(request: UsageHistoryRequest): Promise<UsageHistorySeries[]> {
  return invoke<UsageHistorySeries[]>("get_usage_history", { request });
}
