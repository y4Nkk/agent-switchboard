import type { UpdateChannel, UpdateCheck } from "../api/updates.ts";
import type { MessageKey } from "../i18n/messages.ts";
import { updateDownloadView, type UpdateDownloadDisplayPhase, type UpdateDownloadSnapshot } from "./update-download-state.ts";

export interface ApplicationUpdateSnapshot extends UpdateDownloadSnapshot {
  updateCheck: UpdateCheck | null;
  updateChannel: UpdateChannel | null;
  channelError: unknown;
  appVersion: string | null;
  checking: boolean;
  checkError: unknown;
  lastCheckedAt: string | null;
  restartRequired: boolean;
  restarting: boolean;
  restartError: unknown;
}

export interface ApplicationUpdateControl {
  state: ApplicationUpdateSnapshot;
  onCheck: () => void;
  onInstall: () => void;
  onCancel: () => void;
  onRestart: () => void;
}

export type ApplicationUpdatePhase = UpdateDownloadDisplayPhase | "idle" | "initializing" | "channelFailed"
  | "checking" | "available" | "current" | "checkFailed" | "restarting" | "storeManaged";

export const UPDATE_PHASE_LABELS = {
  idle: "update.check", initializing: "update.initializing", channelFailed: "update.channelFailed",
  checking: "update.checking", available: "update.button", current: "update.current",
  starting: "update.starting", downloading: "update.downloading", verifying: "update.verifying",
  cancelling: "update.cancelling", installing: "update.installing", installed: "update.restartRequired",
  cancelled: "update.cancelled", failed: "update.failed", checkFailed: "update.checkFailed",
  restarting: "update.restarting", storeManaged: "update.storeManaged",
} as const satisfies Record<ApplicationUpdatePhase, MessageKey>;

function phaseOf(state: ApplicationUpdateSnapshot, download: ReturnType<typeof updateDownloadView>): ApplicationUpdatePhase {
  if (state.updateChannel === null) return state.channelError != null ? "channelFailed" : "initializing";
  if (state.updateChannel === "microsoftStore") return "storeManaged";
  if (state.restarting) return "restarting";
  if (state.restartRequired || state.task?.phase === "installed") return "installed";
  if (state.checking) return "checking";
  if (state.checkError != null) return "checkFailed";
  return download.phase ?? (state.updateCheck ? "available" : state.lastCheckedAt ? "current" : "idle");
}

/** Both update surfaces consume this projection; phase and freshness are independent facts. */
export function applicationUpdateView(state: ApplicationUpdateSnapshot) {
  const download = updateDownloadView(state);
  const phase = phaseOf(state, download);
  const busy = phase === "initializing" || phase === "checking" || phase === "restarting" || download.active;
  const installed = phase === "installed" || phase === "restarting";
  const githubReady = state.updateChannel === "github" && !busy && !installed;
  const error = phase === "channelFailed" ? state.channelError : phase === "checkFailed" ? state.checkError
    : phase === "installed" ? state.restartError : phase === "failed" ? download.error : null;
  const warning = download.readError ?? download.cancelError;
  const tone = error != null && !installed ? "danger" : warning != null || error != null ? "warning"
    : phase === "available" || installed ? "safe" : busy ? "action" : "neutral";
  return { phase, busy, installed, download, error, tone,
    canCheck: githubReady || phase === "channelFailed",
    canInstall: githubReady && state.updateCheck !== null && state.checkError == null,
    canRestart: phase === "installed" && state.restartRequired,
    showDetails: !["idle", "current", "checking", "initializing"].includes(phase),
    version: state.task?.version ?? state.updateCheck?.latestVersion ?? null };
}
