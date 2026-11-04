import type { ModelUsageProgress } from "../api/model-usage.ts";

/** Percentages always name the current measured operation, not elapsed time. */
export function modelUsageProgress(progress: ModelUsageProgress | undefined) {
  if (!progress || progress.phase === "discovering" || progress.phase === "publishing") return null;
  const metric = progress.totalBytes !== null
    ? { unit: "bytes" as const, done: progress.completedBytes, total: progress.totalBytes }
    : progress.totalCalls !== null
      ? { unit: "calls" as const, done: progress.completedCalls, total: progress.totalCalls }
      : progress.totalSources !== null
        ? { unit: "sources" as const, done: progress.completedSources, total: progress.totalSources }
        : null;
  if (!metric || metric.total <= 0) return null;
  return { ...metric, percent: Math.min(100, Math.floor(metric.done * 100 / metric.total)) };
}
