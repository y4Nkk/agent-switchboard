import { invoke } from "./client";
import type { AppKind } from "./shared";

/** Local-calendar range for the read-only client session usage report. */
export type ModelUsageRange = "today" | "last7Days" | "last30Days" | "all";

/** One local-session token total, grouped by client and model. It is not a
 * provider billing quota and does not assert a remaining allowance. */
export interface ModelUsageGroup {
  app: AppKind;
  model: string | null;
  inputTokens: number;
  cacheReadInputTokens: number;
  cacheCreationInputTokens: number;
  outputTokens: number;
  totalTokens: number;
  sessionCount: number;
}

/** Token totals assigned to one local-calendar day. These values remain
 * separate from provider billing or quota data. */
export interface ModelUsageDay {
  date: string;
  inputTokens: number;
  cacheReadInputTokens: number;
  cacheCreationInputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

/** Token totals that a local session log did not timestamp. They remain in
 * the exact totals but never become an invented trend point. */
export interface ModelUsageTokens {
  inputTokens: number;
  cacheReadInputTokens: number;
  cacheCreationInputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

/** A non-fatal client-local reason why a model usage report is incomplete. */
export interface ModelUsageIssue {
  app: AppKind;
  message: string;
}

/** Credential-free aggregation over the supported local session records. */
export interface ModelUsageReport {
  range: ModelUsageRange;
  generatedAt: string;
  groups: ModelUsageGroup[];
  days: ModelUsageDay[];
  unassignedTokens: ModelUsageTokens;
  issues: ModelUsageIssue[];
  coverage: { sourceSessions: number; scannedFiles: number; sessionsWithoutUsage: number; undatedEventTokens: number };
}

/** Explicit local-session report cache policy. `forceRefresh` re-scans the
 * approved local session roots instead of returning their saved snapshot. */
export interface ModelUsageRequest {
  range: ModelUsageRange;
  forceRefresh: boolean;
}

/** Whether the report came from its persisted local-session snapshot or this
 * request's completed scan. It does not describe provider quota freshness. */
export type ModelUsageFreshness = "cached" | "fresh";

/** Backend-owned local-session snapshot timing. The UI schedules its next
 * foreground refresh from `refreshAfter`, not from a separate client TTL. */
export interface ModelUsageRead {
  report: ModelUsageReport;
  freshness: ModelUsageFreshness;
  refreshAfter: string;
}

/** Reads the shared local-session snapshot or revalidates approved source
 * revisions and rebuilds range totals from indexed call facts. */
export function getModelUsageReport(request: ModelUsageRequest): Promise<ModelUsageRead> {
  return invoke<ModelUsageRead>("get_model_usage_report", { request });
}
