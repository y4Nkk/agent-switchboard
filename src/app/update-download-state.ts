import type { UpdateDownloadPhase, UpdateDownloadState } from "../api/updates.ts";
import type { CommandError } from "../api/shared.ts";
import type { MessageKey } from "../i18n/messages.ts";

export interface UpdateDownloadSnapshot {
  task: UpdateDownloadState | null;
  starting: boolean;
  cancelling: boolean;
  startError: CommandError | null;
  readError: unknown;
  cancelError: unknown;
}

export type UpdateDownloadDisplayPhase = UpdateDownloadPhase | "starting";

export const UPDATE_DOWNLOAD_LABELS = {
  starting: "backup.update.starting", downloading: "backup.update.downloading", verifying: "backup.update.verifying",
  cancelling: "backup.update.cancelling", installing: "backup.update.installing", installed: "backup.update.restartRequired",
  cancelled: "backup.update.cancelled", failed: "backup.update.failed",
} as const satisfies Record<UpdateDownloadDisplayPhase, MessageKey>;

/** A local cancel request cannot override committed installation or a native terminal result. */
export function updateDownloadPhase(view: UpdateDownloadSnapshot): UpdateDownloadDisplayPhase | null {
  const phase = view.task?.phase;
  if (view.cancelling && (!phase || phase === "downloading" || phase === "verifying")) return "cancelling";
  if (phase) return phase;
  if (view.starting) return "starting";
  return view.startError ? "failed" : null;
}

export function updateDownloadView(state: UpdateDownloadSnapshot) {
  const phase = updateDownloadPhase(state);
  const active = phase === "starting" || (phase !== null && updateDownloadActive(phase));
  const canCancel = phase === "starting" || phase === "downloading" || phase === "verifying" || phase === "cancelling";
  const stale = active && state.task !== null && state.readError != null;
  return { phase, active, canCancel, stale,
    percent: phase === "downloading" && state.task ? downloadPercent(state.task) : null,
    retry: phase === "failed" || phase === "cancelled",
    error: phase === "failed" ? state.task?.error ?? state.startError : null,
    cancelError: canCancel && phase !== "cancelling" ? state.cancelError : null,
    readError: stale ? state.readError : null };
}

export function formatUpdateBytes(bytes: number, language: string): string {
  const units = ["B", "KB", "MB", "GB"];
  const unit = bytes > 0 ? Math.min(3, Math.floor(Math.log10(bytes) / 3)) : 0;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(bytes / 1000 ** unit)} ${units[unit]}`;
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
    ? Math.max(0, Math.min(100, Math.floor(state.downloadedBytes / state.totalBytes * 100))) : null;
}
