import type { CodexProbeObservation } from "../api/codex-probe.ts";

/** Clock ticks do not invalidate immutable answers or re-fetch full details. */
export function probeDetailRevision(status: CodexProbeObservation | null): string {
  return JSON.stringify(status && [status.batchId, status.status, status.completedRuns,
    status.persistPending, status.persistError, status.statusError, status.progress?.kind, status.progress?.seq]);
}
