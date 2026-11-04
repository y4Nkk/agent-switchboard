import { invoke } from "./client";
import type { AppKind, UpstreamProtocol } from "./shared";

export type RequestUsageRange = "last15Minutes" | "lastHour" | "last24Hours" | "last7Days" | "last30Days" | "all";
export type RequestOutcome = "completed" | "failed" | "disconnected";
export type RequestModelSource = "response" | "mapped" | "request" | "unknown";
export interface RequestUsageQuery {
  range: RequestUsageRange;
  app: AppKind | null;
  profileId: string | null;
  model: string | null;
  page: number;
  groupPage: number;
}
export interface RequestUsageRow {
  id: string;
  atMs: number;
  app: AppKind;
  profileId: string;
  model: string | null;
  modelSource: RequestModelSource;
  requestModel: string | null;
  mappedModel: string | null;
  responseModel: string | null;
  upstreamProtocol: UpstreamProtocol | null;
  outcome: RequestOutcome;
  status: number | null;
  durationMs: number;
  firstOutputMs: number | null;
  outputTokensPerSecond: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheCreationTokens: number | null;
  reasoningTokens: number | null;
  estimatedCostUsd: string | null;
  pricingError: string | null;
  attempts: Array<{ profileId: string; status: number | null }>;
}
export interface RequestUsageMetrics {
  requests: number;
  completed: number;
  failed: number;
  disconnected: number;
  incomplete: number;
  completionRate: number | null;
  outputRateP50: number | null;
  outputRateP10: number | null;
  speedSamples: number;
  firstOutputP50Ms: number | null;
  firstOutputP95Ms: number | null;
  firstOutputSamples: number;
  durationP50Ms: number | null;
  durationP95Ms: number | null;
  durationSamples: number;
  cacheHitRate: number | null;
  cacheSamples: number;
  retryRequests: number;
  retries: number;
  attemptSamples: number;
  recoveredRequests: number;
  rateLimited: number;
  upstreamErrors: number;
  unclassifiedFailures: number;
  estimatedCostUsd: string | null;
  costSamples: number;
  pricingErrors: number;
  tokens: Array<{ kind: "input" | "cacheRead" | "cacheCreation" | "output" | "reasoning"; tokens: number | null; samples: number }>;
}
export interface RequestUsageGroup {
  app: AppKind;
  profileId: string;
  model: string | null;
  modelSource: RequestModelSource;
  metrics: RequestUsageMetrics;
}
/** Independent gateway ledger coverage; never added to session consumption totals. */
export interface RequestUsageReport {
  generatedAtMs: number;
  startAtMs: number;
  endAtMs: number;
  bucketMs: number;
  summary: RequestUsageMetrics;
  sources: Array<{ app: AppKind; verifiedRequests: number; earliestAtMs: number | null;
    latestAtMs: number | null; retentionLimit: number | null; error: string | null }>;
  groups: RequestUsageGroup[];
  totalGroups: number;
  groupPage: number;
  groupPageSize: number;
  trend: Array<{ atMs: number; requests: number; speedP50: number | null;
    firstOutputP50Ms: number | null; failureRate: number | null }>;
  rows: RequestUsageRow[];
  page: number;
  pageSize: number;
}

export function readRequestUsage(query: RequestUsageQuery): Promise<RequestUsageReport> {
  return invoke("read_request_usage", { query });
}
