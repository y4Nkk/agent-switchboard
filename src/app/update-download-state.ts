import type { UpdateDownloadPhase, UpdateDownloadState } from "../api/updates.ts";

export interface UpdateDownloadView {
  task: UpdateDownloadState | null;
  starting: boolean;
  cancelling: boolean;
  readError: string | null;
}

export function updateDownloadActive(phase: UpdateDownloadPhase): boolean {
  return phase === "downloading" || phase === "verifying" || phase === "cancelling" || phase === "installing";
}

/** Polls and cancel acknowledgements may arrive in either order. */
export function latestDownloadState(current: UpdateDownloadState | null, next: UpdateDownloadState): UpdateDownloadState {
  return current && (current.id !== next.id || current.revision >= next.revision) ? current : next;
}

export function downloadPercent(state: UpdateDownloadState): number | null {
  return state.totalBytes !== null && state.totalBytes > 0
    ? Math.min(100, Math.floor(state.downloadedBytes / state.totalBytes * 100)) : null;
}
