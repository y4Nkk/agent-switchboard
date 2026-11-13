import assert from "node:assert/strict";
import { test } from "node:test";
import { applicationUpdateView } from "../src/app/update-state.ts";
import { formatUpdateBytes, updateDownloadPhase } from "../src/app/update-download-state.ts";
import { candidate, cancelError, restartError, updateError, updateSnapshot, updateTask } from "./fixtures/update-state.ts";

for (const phase of ["downloading", "verifying", "cancelling", "installing", "installed", "cancelled", "failed"] as const) {
  test(`sidebar reports native ${phase} without turning download bytes into installation progress`, () => {
    const task = updateTask(phase);
    const view = applicationUpdateView(updateSnapshot({ task }));
    assert.equal(view.phase, phase);
    assert.equal(view.showDetails, true);
    assert.equal(view.download.percent, phase === "downloading" ? 26 : null);
    assert.equal(view.busy, ["downloading", "verifying", "cancelling", "installing"].includes(phase));
    assert.equal(view.version, task.version);
  });
}

test("idle, successful check, candidate and checking have distinct actions", () => {
  const idle = applicationUpdateView(updateSnapshot());
  assert.equal(idle.phase, "idle");
  assert.equal(idle.showDetails, false);
  assert.equal(applicationUpdateView(updateSnapshot({ lastCheckedAt: candidate.checkedAt })).phase, "current");
  const available = applicationUpdateView(updateSnapshot({ updateCheck: candidate }));
  assert.equal(available.phase, "available");
  assert.equal(available.tone, "safe");
  assert.equal(available.version, candidate.latestVersion);
  assert.equal(available.showDetails, true);
  assert.equal(applicationUpdateView(updateSnapshot({ checking: true, task: updateTask("failed") })).phase, "checking");
});

test("unknown and zero totals never claim zero or guessed progress", () => {
  for (const totalBytes of [null, 0]) {
    const view = applicationUpdateView(updateSnapshot({ task: updateTask("downloading", { totalBytes }) }));
    assert.equal(view.phase, "downloading");
    assert.equal(view.download.percent, null);
    assert.equal(view.busy, true);
  }
});

test("zero and complete download bytes retain the downloading phase until native transition", () => {
  for (const [downloadedBytes, percent] of [[0, 0], [48_200_000, 100], [50_000_000, 100]]) {
    const view = applicationUpdateView(updateSnapshot({ task: updateTask("downloading", { downloadedBytes }) }));
    assert.equal(view.phase, "downloading");
    assert.equal(view.download.percent, percent);
  }
});

test("pre-start and in-flight cancellation are truthful but cannot override installation or terminal results", () => {
  const before = updateSnapshot({ starting: true, cancelling: true });
  assert.equal(updateDownloadPhase(before), "cancelling");
  assert.equal(applicationUpdateView(before).phase, "cancelling");
  for (const phase of ["downloading", "verifying"] as const) {
    assert.equal(applicationUpdateView(updateSnapshot({ task: updateTask(phase), cancelling: true })).phase, "cancelling");
  }
  for (const phase of ["installing", "installed", "failed", "cancelled"] as const) {
    const state = updateSnapshot({ task: updateTask(phase), cancelling: true });
    assert.equal(updateDownloadPhase(state), phase);
    assert.equal(applicationUpdateView(state).phase, phase);
  }
});

test("read failure retains confirmed bytes but labels the status stale; recovery restores the native phase", () => {
  for (const phase of ["downloading", "verifying", "cancelling", "installing"] as const) {
    const task = updateTask(phase);
    const state = updateSnapshot({ task, readError: "fixture read failure" });
    const view = applicationUpdateView(state);
    assert.equal(view.phase, phase);
    assert.equal(view.download.stale, true);
    assert.equal(view.tone, "warning");
    assert.equal(view.download.percent, phase === "downloading" ? 26 : null);
    assert.equal(state.task, task);
    assert.equal(applicationUpdateView({ ...state, readError: null }).phase, phase);
  }
  const unknown = updateSnapshot({ task: updateTask("downloading", { totalBytes: null }), readError: "fixture" });
  assert.equal(applicationUpdateView(unknown).download.percent, null);
  assert.equal(applicationUpdateView(updateSnapshot({ task: updateTask("installed"), readError: "fixture" })).phase, "installed");
});

test("start and check failures survive without fabricating a download task or hiding the new check error", () => {
  const start = updateSnapshot({ updateCheck: candidate, startError: updateError });
  assert.equal(updateDownloadPhase(start), "failed");
  assert.equal(applicationUpdateView(start).phase, "failed");
  assert.equal(applicationUpdateView(start).showDetails, true);
  assert.equal(start.task, null);
  const failedCheck = applicationUpdateView(updateSnapshot({ task: updateTask("cancelled"), checkError: updateError }));
  assert.equal(failedCheck.phase, "checkFailed");
  assert.equal(failedCheck.showDetails, true);
});

test("installed and restarting remain visible after the candidate is cleared", () => {
  assert.equal(applicationUpdateView(updateSnapshot({ task: updateTask("installed") })).canRestart, false);
  assert.equal(applicationUpdateView(updateSnapshot({ restartRequired: true })).phase, "installed");
  assert.equal(applicationUpdateView(updateSnapshot({ restartRequired: true, restarting: true })).phase, "restarting");
  assert.equal(applicationUpdateView(updateSnapshot({ task: updateTask("installed"), checkError: updateError })).phase, "installed");
});

test("Store updates open details and never offer a manual check or download progress", () => {
  const view = applicationUpdateView(updateSnapshot({ updateChannel: "microsoftStore" }));
  assert.equal(view.phase, "storeManaged");
  assert.equal(view.showDetails, true);
  assert.equal(view.busy, false);
  assert.equal(view.download.percent, null);
});

test("preparing and fresh retry snapshots carry no old task progress", () => {
  const starting = applicationUpdateView(updateSnapshot({ updateCheck: candidate, starting: true }));
  assert.equal(starting.phase, "starting");
  assert.equal(starting.download.percent, null);
  assert.equal(applicationUpdateView(updateSnapshot({ task: updateTask("downloading", { id: "retry", downloadedBytes: 0 }) })).download.percent, 0);
});

test("sidebar and About format actual bytes with the same decimal units", () => {
  assert.equal(formatUpdateBytes(12_600_000, "zh-CN"), "12.6 MB");
  assert.equal(formatUpdateBytes(48_200_000, "en-US"), "48.2 MB");
  assert.equal(formatUpdateBytes(0, "en-US"), "0 B");
});

test("an unresolved channel blocks commands; a channel failure can be retried without assuming GitHub", () => {
  const waiting = applicationUpdateView(updateSnapshot({ updateChannel: null, updateCheck: candidate }));
  assert.equal(waiting.phase, "initializing");
  assert.equal(waiting.busy, true);
  assert.equal(waiting.canCheck, false);
  assert.equal(waiting.canInstall, false);
  const failed = applicationUpdateView(updateSnapshot({ updateChannel: null, channelError: updateError }));
  assert.equal(failed.phase, "channelFailed");
  assert.equal(failed.error, updateError);
  assert.equal(failed.canCheck, true);
  assert.equal(failed.canInstall, false);
  assert.equal(failed.showDetails, true);
});

test("cancellation failure preserves the actual running task and a usable cancellation action", () => {
  const state = updateSnapshot({ task: updateTask(), cancelError });
  const view = applicationUpdateView(state);
  assert.equal(view.phase, "downloading");
  assert.equal(view.download.percent, 26);
  assert.equal(view.download.stale, false);
  assert.equal(view.download.cancelError, cancelError);
  assert.equal(view.download.canCancel, true);
  assert.equal(view.tone, "warning");
  assert.equal(view.canInstall, false);
});

test("terminal results discard old read/cancel failures and installation never offers cancellation", () => {
  for (const phase of ["installed", "cancelled", "failed"] as const) {
    const view = applicationUpdateView(updateSnapshot({ task: updateTask(phase), readError: updateError, cancelError }));
    assert.equal(view.download.readError, null);
    assert.equal(view.download.cancelError, null);
    assert.equal(view.download.stale, false);
    assert.equal(view.download.canCancel, false);
  }
  assert.equal(applicationUpdateView(updateSnapshot({ task: updateTask("installing"), cancelError })).download.canCancel, false);
});

test("restart failure preserves installed state and only allows the restart operation", () => {
  const view = applicationUpdateView(updateSnapshot({ restartRequired: true, restartError }));
  assert.equal(view.phase, "installed");
  assert.equal(view.error, restartError);
  assert.equal(view.canRestart, true);
  assert.equal(view.canInstall, false);
  assert.equal(view.canCheck, false);
  const pending = applicationUpdateView(updateSnapshot({ restartRequired: true, restarting: true, restartError }));
  assert.equal(pending.phase, "restarting");
  assert.equal(pending.canRestart, false);
  assert.equal(pending.error, null);
});
