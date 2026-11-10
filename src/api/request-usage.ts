import { invoke } from "./client";
import type { AppKind } from "./shared";

export type RequestUsageRange = "last15Minutes" | "lastHour" | "last24Hours" | "last7Days" | "last30Days" | "all";
export type MeasurementScope = "generation" | "transport";
export type RequestOutcome = "succeeded" | "failed" | "unknown";
export interface RequestUsageQuery {
  range: RequestUsageRange;
  app: AppKind | null;
  scope: MeasurementScope;
  model: string | null;
  page: number;
  groupPage: number;
}
export interface RequestUsageRow {
  id: string;
  atMs: number;
  timeSource: "event" | "observed" | "spanEnd";
  app: AppKind;
  scope: MeasurementScope;
  eventName: string;
  eventKind: string | null;
  model: string | null;
  sessionId: string | null;
  requestId: string | null;
  outcome: RequestOutcome;
  status: number | null;
  errorType: string | null;
  durationMs: number | null;
  firstTokenMs: number | null;
  firstContentMs: number | null;
  outputTokensPerSecond: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheCreationTokens: number | null;
  reasoningTokens: number | null;
  estimatedCostUsd: string | null;
  attemptCount: number | null;
}
export interface RequestUsageMetrics {
  records: number;
  succeeded: number;
  failed: number;
  unknown: number;
  mixedMeasurements: boolean;
  successRatio: number | null;
  outputRateP50: number | null;
  outputRateP10: number | null;
  speedSamples: number;
  firstTokenP50Ms: number | null;
  firstTokenP95Ms: number | null;
  firstTokenSamples: number;
  firstContentP50Ms: number | null;
  firstContentSamples: number;
  durationP50Ms: number | null;
  durationP95Ms: number | null;
  durationSamples: number;
  cacheHitRate: number | null;
  cacheSamples: number;
  retryRecords: number;
  retries: number;
  attemptSamples: number;
  recoveredRecords: number;
  rateLimited: number;
  upstreamErrors: number;
  unclassifiedFailures: number;
  estimatedCostUsd: string | null;
  costSamples: number;
  tokens: Array<{ kind: "input" | "cacheRead" | "cacheCreation" | "output" | "reasoning"; tokens: number | null; samples: number }>;
}
export interface RequestUsageGroup {
  app: AppKind;
  model: string | null;
  eventName: string;
  metrics: RequestUsageMetrics;
}
/** Native observations only. Generation and transport scopes never share a denominator. */
export interface RequestUsageReport {
  generatedAtMs: number;
  startAtMs: number;
  endAtMs: number;
  bucketMs: number;
  scope: MeasurementScope;
  summary: RequestUsageMetrics;
  sources: Array<{ app: AppKind; verifiedRecords: number; earliestAtMs: number | null; latestAtMs: number | null }>;
  groups: RequestUsageGroup[];
  totalGroups: number;
  groupPage: number;
  groupPageSize: number;
  trend: Array<{ atMs: number; speedP50: number | null; firstTokenP50Ms: number | null;
    durationP50Ms: number | null; failureRate: number | null }>;
  rows: RequestUsageRow[];
  page: number;
  pageSize: number;
}

export function readRequestUsage(query: RequestUsageQuery): Promise<RequestUsageReport> {
  return invoke("read_request_usage", { query });
}
