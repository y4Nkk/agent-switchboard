import assert from "node:assert/strict";
import { test } from "node:test";
import type { CodexProbeBatch, CodexProbeObservation } from "../src/api/codex-probe.ts";
import { probeDetailRevision } from "../src/app/probe-observation.ts";
import { startProbeDetailReading } from "../src/components/probe-detail-reader.ts";
import { createReadQueue } from "../src/hooks/use-read-queue.ts";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const settle = () => new Promise<void>((resolve) => setImmediate(resolve));
function fixture(batchId = "one"): CodexProbeBatch {
  return { batchId, startedAt: "2026-11-03T00:00:00Z", finishedAt: "2026-11-03T00:00:01Z",
    status: "completed", plannedRuns: 1, completedRuns: 0, gradingVersion: "1", cliVersion: null,
    question: { id: "custom", label: "question", text: "text", expectedAnswer: "42" },
    config: { profileId: null, profileName: null, profileModel: null, reasoningEffort: null,
      connectionIdentity: null, fingerprint: "fixture" }, runs: [], statusError: null,
    persistPending: false, persistError: null, progress: null };
}

test("隐藏的详情不会接收迟到全文，停止后刷新不会重新读取", async () => {
  const pending = deferred<CodexProbeBatch | null>();
  const values: Array<CodexProbeBatch | null> = [];
  let reads = 0;
  const reader = startProbeDetailReading({ batchId: "one", enqueue: createReadQueue(),
    read: () => { reads++; return pending.promise; }, onRead: (value) => values.push(value),
    onLoading: () => {}, onError: () => assert.fail("unexpected error") });
  reader.stop(); reader.refresh();
  pending.resolve(fixture());
  await settle();
  assert.equal(reads, 1);
  assert.deepEqual(values, []);
});

test("慢读取期间连续刷新只保留最新一次待执行读取", async () => {
  const pending = deferred<CodexProbeBatch | null>();
  let reads = 0;
  const values: Array<CodexProbeBatch | null> = [];
  const reader = startProbeDetailReading({ batchId: "one", enqueue: createReadQueue(),
    read: async () => ++reads === 1 ? pending.promise : fixture(),
    onRead: (value) => values.push(value), onLoading: () => {}, onError: () => assert.fail("unexpected error") });
  for (let index = 0; index < 1000; index++) reader.refresh();
  pending.resolve(fixture("obsolete"));
  await settle();
  assert.equal(reads, 2);
  assert.deepEqual(values.map((value) => value?.batchId), ["one"]);
  reader.stop();
});

test("切换详情和隐藏后恢复复用有界队列，不接受旧批次", async () => {
  const enqueue = createReadQueue();
  const pending = deferred<CodexProbeBatch | null>();
  const values: string[] = [];
  const options = { enqueue, onRead: (value: CodexProbeBatch | null) => { if (value) values.push(value.batchId); },
    onLoading: () => {}, onError: () => assert.fail("unexpected error") };
  const old = startProbeDetailReading({ ...options, batchId: "old", read: () => pending.promise });
  old.stop();
  const restored = startProbeDetailReading({ ...options, batchId: "new", read: async () => fixture("new") });
  pending.resolve(fixture("old"));
  await settle();
  assert.deepEqual(values, ["new"]);
  restored.stop();
});

test("读取失败后可重试，空结果作为缺失详情交给页面表达", async () => {
  let reads = 0;
  const errors: unknown[] = [];
  const values: Array<CodexProbeBatch | null> = [];
  const reader = startProbeDetailReading({ batchId: "one", enqueue: createReadQueue(),
    read: async () => { if (++reads === 1) throw new Error("fixture failure"); return null; },
    onRead: (value) => values.push(value), onLoading: () => {}, onError: (error) => errors.push(error) });
  await settle(); reader.refresh(); await settle();
  assert.equal(errors.length, 1);
  assert.deepEqual(values, [null]);
  reader.stop();
});

test("后台时钟变化不重读全文，轮次、完成及保存变化使详情失效", () => {
  const status: CodexProbeObservation = { batchId: "one", status: "running", plannedRuns: 2,
    completedRuns: 0, passedRuns: 0, judgedRuns: 0, executionFailed: false, questionLabel: "question",
    gradingVersion: "1", statusError: null, persistPending: false, persistError: null,
    progress: { kind: "run", seq: 1, phase: "running", elapsedMs: 100, timeLimitMs: 1000 } };
  const key = probeDetailRevision(status);
  assert.equal(probeDetailRevision({ ...status, progress: { kind: "run", seq: 1,
    phase: "saving", elapsedMs: 900, timeLimitMs: 1000 } }), key);
  assert.notEqual(probeDetailRevision({ ...status, completedRuns: 1 }), key);
  assert.notEqual(probeDetailRevision({ ...status, progress: { kind: "preparing", seq: 2, cancelling: false } }), key);
  assert.notEqual(probeDetailRevision({ ...status, status: "failed", progress: null,
    persistPending: true, persistError: "pending" }), key);
});
