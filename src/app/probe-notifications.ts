import type { CodexProbeBatch } from "../api/usage";
import { toastMessage, type Toast } from "../components/use-toast.ts";

export interface ProbeNotificationState {
  watching: string | null;
  delivered: { batchId: string; persistPending: boolean } | null;
}

type Notice = Omit<Toast, "id">;
type TerminalBatch = Exclude<CodexProbeBatch, { status: "running" }>;

function terminalNotice(batch: TerminalBatch): Notice {
  if (batch.persistPending) return { kind: "error", durationMs: 0,
    title: toastMessage("codex.probe.notify.unsaved"),
    description: batch.persistError ?? toastMessage("codex.probe.persistPending") };
  if (batch.status === "completed") {
    const judged = batch.runs.filter((run) => run.status === "passed" || run.status === "failed");
    const passed = judged.filter((run) => run.status === "passed").length;
    const issues = batch.runs.some((run) => run.executionError !== null)
      || (batch.gradingVersion === "1" && passed !== batch.plannedRuns);
    return { kind: issues ? "warning" : "success", title: toastMessage("codex.probe.notify.completed"),
      description: batch.gradingVersion === "1"
        ? toastMessage("codex.probe.notify.graded", { question: batch.question.label,
          passed, judged: judged.length, total: batch.plannedRuns })
        : toastMessage(issues ? "codex.probe.notify.visualIssues" : "codex.probe.notify.visual",
          { question: batch.question.label }) };
  }
  const titles = { cancelled: "codex.probe.notify.cancelled", "config-changed": "codex.probe.notify.configChanged",
    interrupted: "codex.probe.notify.interrupted", failed: "codex.probe.notify.failed" } as const;
  const title = titles[batch.status];
  return { kind: batch.status === "cancelled" ? "info" : "error", title: toastMessage(title),
    description: batch.statusError ?? batch.question.label };
}

/** Only an observed live task or explicit lifecycle signal may notify.
 * Browsing/deleting history must never turn an older batch into a new result. */
export function advanceProbeNotification(state: ProbeNotificationState, batch: CodexProbeBatch | null): {
  state: ProbeNotificationState; notice: Notice | null;
} {
  if (!batch) return { state, notice: null };
  if (batch.status === "running") return { state: { ...state, watching: batch.batchId }, notice: null };
  if (!batch.persistPending && state.watching !== batch.batchId) return { state, notice: null };
  const delivered = { batchId: batch.batchId, persistPending: batch.persistPending };
  const next = { watching: batch.persistPending ? batch.batchId : null, delivered };
  if (state.delivered?.batchId === batch.batchId && state.delivered.persistPending === batch.persistPending) {
    return { state: next, notice: null };
  }
  const saved = state.delivered?.batchId === batch.batchId && state.delivered.persistPending;
  return { state: next, notice: saved && !batch.persistPending
    ? { kind: "success", title: toastMessage("codex.probe.notify.saved"), description: batch.question.label }
    : terminalNotice(batch) };
}
