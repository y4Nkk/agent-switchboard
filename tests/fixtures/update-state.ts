import type { CommandError } from "../../src/api/shared.ts";
import type { UpdateDownloadState } from "../../src/api/updates.ts";
import type { ApplicationUpdateSnapshot } from "../../src/app/update-state.ts";

export const candidate = { id: "release-one", latestVersion: "0.3.24", releaseNotes: null,
  checkedAt: "2026-11-13T06:53:00Z" };
export const updateError: CommandError = { code: "fixture-updater-failure", message: "fixture connection refused",
  messageKey: "common.operationFailed" };

export const readError = { code: "fixture-read", message: "fixture status unavailable", messageKey: "common.operationFailed" };
export const cancelError = { code: "fixture-cancel", message: "fixture cancellation refused", messageKey: "common.operationFailed" };
export const restartError = { code: "fixture-restart", message: "fixture restart refused", messageKey: "common.operationFailed" };

export function updateTask(phase: UpdateDownloadState["phase"] = "downloading",
  patch: Partial<UpdateDownloadState> = {}): UpdateDownloadState {
  return { id: "download-one", revision: 1, version: candidate.latestVersion, phase,
    downloadedBytes: 12_600_000, totalBytes: 48_200_000, error: null, ...patch };
}

export function updateSnapshot(patch: Partial<ApplicationUpdateSnapshot> = {}): ApplicationUpdateSnapshot {
  return { updateChannel: "github", channelError: null, appVersion: "0.3.23", updateCheck: null, checking: false, checkError: null, lastCheckedAt: null,
    task: null, starting: false, cancelling: false, startError: null, readError: null, cancelError: null,
    restartRequired: false, restarting: false, restartError: null, ...patch };
}

export function updateExamples(): [string, ApplicationUpdateSnapshot][] {
  const native = (phase: UpdateDownloadState["phase"]) => updateSnapshot({ task: updateTask(phase) });
  return [
    ["idle", updateSnapshot()],
    ["initializing", updateSnapshot({ updateChannel: null })],
    ["channel-failed", updateSnapshot({ updateChannel: null, channelError: updateError })],
    ["checking", updateSnapshot({ checking: true })],
    ["current", updateSnapshot({ lastCheckedAt: candidate.checkedAt })],
    ["available", updateSnapshot({ updateCheck: candidate })],
    ["starting", updateSnapshot({ updateCheck: candidate, starting: true })],
    ["cancel-before-start", updateSnapshot({ starting: true, cancelling: true })],
    ["download-zero", updateSnapshot({ task: updateTask("downloading", { downloadedBytes: 0 }) })],
    ["downloading", native("downloading")],
    ["download-complete", updateSnapshot({ task: updateTask("downloading", { downloadedBytes: 48_200_000 }) })],
    ["unknown-total", updateSnapshot({ task: updateTask("downloading", { totalBytes: null }) })],
    ["zero-total", updateSnapshot({ task: updateTask("downloading", { totalBytes: 0 }) })],
    ["verifying", native("verifying")],
    ["cancelling", native("cancelling")],
    ["cancel-pending", updateSnapshot({ task: updateTask(), cancelling: true })],
    ["installing", native("installing")],
    ["install-wins-cancel", updateSnapshot({ task: updateTask("installing"), cancelling: true })],
    ["installed", native("installed")],
    ["restart-required", updateSnapshot({ restartRequired: true })],
    ["restarting", updateSnapshot({ restartRequired: true, restarting: true })],
    ["cancelled", native("cancelled")],
    ["failed", updateSnapshot({ task: updateTask("failed", { error: updateError }) })],
    ["start-failed", updateSnapshot({ updateCheck: candidate, startError: updateError })],
    ["check-failed", updateSnapshot({ checkError: updateError })],
    ["stale-progress", updateSnapshot({ task: updateTask(), readError })],
    ["stale-unknown", updateSnapshot({ task: updateTask("downloading", { totalBytes: null }), readError })],
    ["stale-verifying", updateSnapshot({ task: updateTask("verifying"), readError })],
    ["stale-installing", updateSnapshot({ task: updateTask("installing"), readError })],
    ["stale-cancelling", updateSnapshot({ task: updateTask("cancelling"), readError })],
    ["stale-cancel-pending", updateSnapshot({ task: updateTask(), cancelling: true, readError })],
    ["cancel-failed", updateSnapshot({ task: updateTask(), cancelError })],
    ["restart-failed", updateSnapshot({ task: updateTask("installed"), restartRequired: true, restartError })],
    ["terminal-discards-old-errors", updateSnapshot({ task: updateTask("cancelled"), readError, cancelError })],
    ["store-managed", updateSnapshot({ updateChannel: "microsoftStore" })],
  ];
}
