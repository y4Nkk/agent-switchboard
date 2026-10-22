import assert from "node:assert/strict";
import { test } from "node:test";
import { createReadQueue } from "../src/hooks/use-read-queue.ts";

function deferred() {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => { release = resolve; });
  return { promise, release };
}

test("慢读取期间只执行当前任务和最新请求", async () => {
  const enqueue = createReadQueue();
  const first = deferred();
  const last = deferred();
  const started: number[] = [];
  const failures: unknown[] = [];
  enqueue(async () => { started.push(0); await first.promise; }, (reason) => failures.push(reason));
  for (let index = 1; index <= 1000; index++) {
    enqueue(async () => { started.push(index); last.release(); }, (reason) => failures.push(reason));
  }
  assert.deepEqual(started, [0]);
  first.release();
  await last.promise;
  assert.deepEqual(started, [0, 1000]);
  assert.deepEqual(failures, []);
});

test("过期生命周期的取消不移除新的待执行任务", async () => {
  const enqueue = createReadQueue();
  const first = deferred();
  const last = deferred();
  const started: string[] = [];
  const failures: unknown[] = [];
  enqueue(async () => { started.push("first"); await first.promise; }, (reason) => failures.push(reason));
  const cancelOld = enqueue(async () => { started.push("old"); }, (reason) => failures.push(reason));
  enqueue(async () => { started.push("latest"); last.release(); }, (reason) => failures.push(reason));
  cancelOld(); first.release();
  await last.promise;
  assert.deepEqual(started, ["first", "latest"]);
  assert.deepEqual(failures, []);
});

test("取消待执行读取后不再读取其数据，执行中的原生操作仍能结束", async () => {
  const enqueue = createReadQueue();
  const first = deferred();
  const last = deferred();
  const started: string[] = [];
  const failures: unknown[] = [];
  const cancelFirst = enqueue(async () => {
    started.push("first"); await first.promise; started.push("first-completed");
  }, (reason) => failures.push(reason));
  const cancelPending = enqueue(async () => { started.push("canceled"); }, (reason) => failures.push(reason));
  cancelPending(); cancelFirst();
  enqueue(async () => { started.push("next"); last.release(); }, (reason) => failures.push(reason));
  first.release();
  await last.promise;
  assert.deepEqual(started, ["first", "first-completed", "next"]);
  assert.deepEqual(failures, []);
});

test("读取失败交给原请求所有者处理，随后继续最新请求", async () => {
  const enqueue = createReadQueue();
  const first = deferred();
  const last = deferred();
  const error = new Error("disk read failed");
  const failures: unknown[] = [];
  enqueue(async () => { await first.promise; throw error; }, (reason) => failures.push(reason));
  enqueue(async () => { last.release(); }, (reason) => failures.push(reason));
  first.release();
  await last.promise;
  assert.deepEqual(failures, [error]);
});
