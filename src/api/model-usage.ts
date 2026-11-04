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

/** Whether the report came from a persisted summary or a completed refresh
 * task. It does not describe provider quota freshness. */
export type ModelUsageFreshness = "cached" | "fresh";

/** Backend-owned local-session snapshot timing. The UI schedules its next
 * foreground refresh from `refreshAfter`, not from a separate client TTL. */
export interface ModelUsageRead {
  reports: ModelUsageReport[];
  freshness: ModelUsageFreshness;
  refreshAfter: string;
}

export type ModelUsagePhase = "discovering" | "checking" | "reading" | "verifying" | "aggregating" | "publishing";

export interface ModelUsageProgress {
  phase: ModelUsagePhase;
  completedSources: number;
  totalSources: number | null;
  completedBytes: number;
  totalBytes: number | null;
  completedCalls: number;
  totalCalls: number | null;
}

export type ModelUsageRefresh =
  | { status: "running"; id: string; startedAt: string; progress: ModelUsageProgress }
  | { status: "completed"; id: string; startedAt: string; read: ModelUsageRead }
  | { status: "failed"; id: string; startedAt: string; message: string };

/** Reads the last verified summaries without discovering or scanning sessions. */
export function readModelUsageSnapshot(): Promise<ModelUsageRead | null> {
  return invoke<ModelUsageRead | null>("read_model_usage_snapshot");
}

/** Starts or joins the backend task. Its terminal result contains all ranges. */
export function refreshModelUsage(): Promise<ModelUsageRefresh> {
  return invoke<ModelUsageRefresh>("refresh_model_usage");
}

export function readModelUsageRefresh(id: string): Promise<ModelUsageRefresh> {
  return invoke<ModelUsageRefresh>("read_model_usage_refresh", { id });
}
