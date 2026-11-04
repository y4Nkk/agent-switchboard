import { invoke } from "./client";

/** `questionId` that selects the user-authored question. */
export const CUSTOM_PROBE_QUESTION_ID = "custom";

export type CodexProbeGradingVersion = "1" | "svg-visual-v1" | "html-visual-v1";

/** One selectable probe question from the backend-owned catalog. */
export interface CodexProbeQuestion {
  id: string;
  label: string;
  text: string;
  gradingVersion: CodexProbeGradingVersion;
  /** A hand-made reference rendering; never a grading baseline. */
  referenceVisual: string | null;
  /** Upper bound of runs per batch for this question. */
  maxRuns: number;
  timeoutSeconds: number;
}

export interface CodexProbeCatalog {
  questions: CodexProbeQuestion[];
  customTimeoutSeconds: Record<CodexProbeGradingVersion, number>;
  customMaxRuns: number;
}

export type CodexProbeRunPhase = "starting" | "running" | "collecting" | "saving" | "cancelling";

/** A snapshot of the runner's monotonic clock, never a frontend deadline. */
export interface CodexProbeRunProgress {
  kind: "run";
  seq: number;
  phase: CodexProbeRunPhase;
  elapsedMs: number;
  timeLimitMs: number;
}

export type CodexProbeProgress = CodexProbeRunProgress
  | { kind: "preparing"; seq: number; cancelling: boolean };

export type CodexProbeBatchStatus =
  | "running" | "completed" | "cancelled" | "failed" | "interrupted" | "config-changed";

/** Automatic verdict; visual answers and execution failures stay undetermined. */
export type CodexProbeRunStatus = "running" | "passed" | "failed" | "undetermined";

/** One run row as persisted: the session id is saved the moment the CLI
 * reports it; unknown usage stays `null`, a recorded real zero stays zero. */
export interface CodexProbeRun {
  seq: number;
  status: CodexProbeRunStatus;
  sessionId: string | null;
  finalAnswer: string | null;
  reportedModel: string | null;
  durationMs: number | null;
  reasoningTokens: number | null;
  totalTokens: number | null;
  executionError: string | null;
  usageError: string | null;
}

/** The configuration snapshot taken when the batch started. Profile fields
 * are snapshots — renames and deletions never rewrite history, and an
 * unidentified configuration shows as 未关联档案. */
export interface CodexProbeConfigSnapshot {
  profileId: string | null;
  profileName: string | null;
  profileModel: string | null;
  reasoningEffort: string | null;
  connectionIdentity: string | null;
  fingerprint: string;
}

export interface CodexProbeQuestionSnapshot {
  id: string;
  label: string;
  text: string;
  expectedAnswer: string;
}

/** One batch (live or historical) with its runs, read from the local probe
 * history — the only durable source of probe state. */
interface CodexProbeBatchRecord {
  batchId: string;
  startedAt: string;
  finishedAt: string | null;
  plannedRuns: number;
  completedRuns: number;
  question: CodexProbeQuestionSnapshot;
  gradingVersion: CodexProbeGradingVersion;
  cliVersion: string | null;
  config: CodexProbeConfigSnapshot;
  statusError: string | null;
  runs: CodexProbeRun[];
  /** True when a persistence failure left results only in memory. */
  persistPending: boolean;
  persistError: string | null;
}

export type CodexProbeBatch = CodexProbeBatchRecord & (
  | { status: "running"; persistPending: false; progress: CodexProbeProgress }
  | { status: Exclude<CodexProbeBatchStatus, "running">; progress: null }
);

/** The application-wide status contains no answer, prompt or configuration. */
interface CodexProbeObservationRecord {
  batchId: string;
  plannedRuns: number;
  completedRuns: number;
  passedRuns: number;
  judgedRuns: number;
  executionFailed: boolean;
  questionLabel: string;
  gradingVersion: CodexProbeGradingVersion;
  statusError: string | null;
  persistPending: boolean;
  persistError: string | null;
}

export type CodexProbeObservation = CodexProbeObservationRecord & (
  | { status: "running"; persistPending: false; progress: CodexProbeProgress }
  | { status: Exclude<CodexProbeBatchStatus, "running">; progress: null }
);

export interface CodexProbeRequest {
  runCount: number;
  /** A catalog id, or `custom` with an explicit format and integer answer when needed. */
  questionId: string;
  customQuestion?: string;
  customAnswer?: string;
  customGradingVersion?: CodexProbeGradingVersion;
}

/** The backend-owned question catalog and custom-format time limits. */
export function getCodexProbeCatalog(): Promise<CodexProbeCatalog> {
  return invoke<CodexProbeCatalog>("get_codex_probe_catalog");
}

/** Starts one probe batch against the active Codex configuration. Every run
 * is a real Codex call and spends quota; the batch is recorded in the local
 * history before the first call. Returns its id so even a batch that ends
 * before the first status read can be observed and notified exactly once. */
export function startCodexProbe(request: CodexProbeRequest): Promise<string> {
  return invoke<string>("start_codex_probe", { request });
}

/** Reconnects to the live-or-latest task without retaining full detail. */
export function getCodexProbeObservation(): Promise<CodexProbeObservation | null> {
  return invoke<CodexProbeObservation | null>("get_codex_probe_observation");
}

/** Requests cancellation of the running batch; the in-flight run is
 * terminated. */
export function cancelCodexProbe(): Promise<boolean> {
  return invoke<boolean>("cancel_codex_probe");
}

/** Flushes results whose persistence failed mid-batch. */
export function retryCodexProbeSave(): Promise<void> {
  return invoke<void>("retry_codex_probe_save");
}

export type CodexProbeHistoryRange = "all" | "last7Days" | "last30Days";

export type CodexProbeProfileFilter =
  | { kind: "all" } | { kind: "unlinked" } | { kind: "profile"; id: string };

export interface CodexProbeHistoryQuery {
  offset: number;
  limit: number;
  range: CodexProbeHistoryRange;
  profile: CodexProbeProfileFilter;
  /** `all` or omitted means every status. */
  status: CodexProbeBatchStatus | "all";
}

/** One row of the history list. */
export interface CodexProbeHistoryItem {
  batchId: string;
  startedAt: string;
  finishedAt: string | null;
  status: CodexProbeBatchStatus;
  questionLabel: string;
  gradingVersion: CodexProbeGradingVersion;
  profileId: string | null;
  profileName: string | null;
  runCount: number;
  passedCount: number;
  judgedCount: number;
  recordedRuns: number;
  totalTokens: number | null;
}

export interface CodexProbeHistoryPage {
  items: CodexProbeHistoryItem[];
  total: number;
  offset: number;
  limit: number;
}

/** History reads are paged and ordered newest-first. */
export function listCodexProbeHistory(query: CodexProbeHistoryQuery): Promise<CodexProbeHistoryPage> {
  return invoke<CodexProbeHistoryPage>("list_codex_probe_history", {
    request: {
      offset: query.offset,
      limit: query.limit,
      days: query.range === "all" ? null : query.range === "last7Days" ? 7 : 30,
      profile: query.profile,
      status: query.status === "all" ? null : query.status,
    },
  });
}

/** Distinct profile snapshots seen in the history, for the filter. */
export function listCodexProbeHistoryProfiles(): Promise<Array<{ profileId: string; profileName: string }>> {
  return invoke<Array<{ profileId: string; profileName: string }>>("list_codex_probe_history_profiles");
}

/** One historical batch with its runs. */
export function getCodexProbeBatch(batchId: string): Promise<CodexProbeBatch | null> {
  return invoke<CodexProbeBatch | null>("get_codex_probe_batch", { batchId });
}

/** Deletes finished batches and their runs — only the radar's own records;
 * Codex sessions and the real usage ledger stay untouched. */
export function deleteCodexProbeBatches(batchIds: string[]): Promise<{ deleted: number; warning: string | null }> {
  return invoke<{ deleted: number; warning: string | null }>("delete_codex_probe_batches", {
    request: { batchIds },
  });
}
