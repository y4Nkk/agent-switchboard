import assert from "node:assert/strict";
import { test } from "node:test";
import type { CodexProbeObservation } from "../src/api/codex-probe.ts";
import { advanceProbeNotification, type ProbeNotificationState } from "../src/app/probe-notifications.ts";

const idle = (): ProbeNotificationState => ({ watching: null, delivered: null });
const watching = (): ProbeNotificationState => ({ ...idle(), watching: "batch" });
function batch(status: Exclude<CodexProbeObservation["status"], "running"> = "completed",
  overrides: Partial<CodexProbeObservation> = {}): CodexProbeObservation {
  return { batchId: "batch", plannedRuns: 1, completedRuns: 1, gradingVersion: "1", statusError: null,
    questionLabel: "Question", passedRuns: 1, judgedRuns: 1, executionFailed: false,
    persistPending: false, persistError: null, ...overrides, status, progress: null };
}

test("初次历史读取、历史删除后切到旧结果都不重播通知", () => {
  assert.equal(advanceProbeNotification(idle(), batch()).notice, null);
  assert.equal(advanceProbeNotification(idle(), null).notice, null);
  const done = advanceProbeNotification(watching(), batch());
  assert.equal(advanceProbeNotification(done.state, batch("completed", { batchId: "older" })).notice, null);
});

test("任务信号覆盖未读到 running 的短任务，重复事件与重新读取不重播", () => {
  const next = advanceProbeNotification(watching(), batch());
  assert.equal(next.notice?.kind, "success");
  assert.deepEqual(next.notice?.title, { __toastMessage: true, key: "codex.probe.notify.completed", params: undefined });
  assert.equal(advanceProbeNotification(next.state, batch()).notice, null);
  assert.equal(advanceProbeNotification({ ...next.state, watching: "batch" }, batch()).notice, null);
});

test("重新连接到运行批次后仍通知完成，读取空值不丢失正在关注的任务", () => {
  const live: CodexProbeObservation = { ...batch(), status: "running", persistPending: false,
    progress: { kind: "preparing", seq: 1, cancelling: false } };
  const observed = advanceProbeNotification(idle(), live);
  assert.equal(observed.notice, null);
  const empty = advanceProbeNotification(observed.state, null);
  assert.equal(advanceProbeNotification(empty.state, batch()).notice?.kind, "success");
});

test("所有结束原因都有各自的全局反馈", () => {
  for (const [status, key, kind] of [
    ["cancelled", "codex.probe.notify.cancelled", "info"],
    ["failed", "codex.probe.notify.failed", "error"],
    ["config-changed", "codex.probe.notify.configChanged", "error"],
    ["interrupted", "codex.probe.notify.interrupted", "error"],
  ] as const) {
    const notice = advanceProbeNotification(watching(), batch(status, { statusError: "diagnostic" })).notice;
    assert.equal(notice?.kind, kind);
    assert.equal(notice?.description, "diagnostic");
    assert.deepEqual(notice?.title, { __toastMessage: true, key, params: undefined });
  }
});

test("判分未通过和执行未判定给出警告，视觉题要求人工观察", () => {
  for (const judgedRuns of [0, 1]) {
    assert.equal(advanceProbeNotification(watching(), batch("completed", { passedRuns: 0, judgedRuns })).notice?.kind, "warning");
  }
  const visual = batch("completed", { gradingVersion: "html-visual-v1", passedRuns: 0, judgedRuns: 0 });
  const notice = advanceProbeNotification(watching(), visual).notice;
  assert.equal(notice?.kind, "success");
  assert.deepEqual(notice?.description, { __toastMessage: true, key: "codex.probe.notify.visual",
    params: { question: "Question" } });
  assert.equal(advanceProbeNotification(watching(), { ...visual,
    executionFailed: true }).notice?.kind, "warning");
});

test("首次待保存结果立即提示，重试失败不重播，提交后只确认保存一次", () => {
  const pending = batch("failed", { persistPending: true, persistError: "disk unavailable" });
  const stopped = advanceProbeNotification(idle(), pending);
  assert.equal(stopped.notice?.kind, "error");
  assert.equal(stopped.notice?.durationMs, 0);
  assert.equal(stopped.notice?.description, "disk unavailable");
  const retry = advanceProbeNotification(stopped.state, pending);
  assert.equal(retry.notice, null);
  const saved = advanceProbeNotification(retry.state, batch("failed"));
  assert.equal(saved.notice?.kind, "success");
  assert.deepEqual(saved.notice?.title, { __toastMessage: true, key: "codex.probe.notify.saved", params: undefined });
  assert.equal(advanceProbeNotification({ ...saved.state, watching: "batch" }, batch("failed")).notice, null);
});

test("连续批次分别通知，上一批结果不阻止下一批结束提示", () => {
  const first = advanceProbeNotification(watching(), batch());
  const next = advanceProbeNotification({ ...first.state, watching: "next" }, batch("completed", { batchId: "next" }));
  assert.equal(next.notice?.kind, "success");
  assert.equal(next.state.delivered?.batchId, "next");
});
