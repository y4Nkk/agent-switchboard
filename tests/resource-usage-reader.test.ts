import assert from "node:assert/strict";
import { test } from "node:test";
import { createReadQueue } from "../src/hooks/use-read-queue.ts";
import { startResourceReading } from "../src/pages/resource-usage-reader.ts";
import type { ResourceRead, ResourceReadRequest, ResourceSelection } from "../src/api/resource-usage.ts";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function reading(id: number): ResourceRead {
  return { sampleId: id, sampledAt: "2026-10-23T00:00:00Z", nextReadInMs: 2_000,
    scope: { platform: "windows", interfaceCoverage: "verified", issues: [] }, groups: [], details: null };
}

async function settle() { for (let index = 0; index < 8; index++) await Promise.resolve(); }

test("visible entry resets the interval once and stop cancels periodic reads", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const requests: ResourceReadRequest[] = [];
  const reads: number[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async (request) => { requests.push(request); return reading(requests.length); },
    onLoading: () => {}, onError: () => {}, onRead: (read) => reads.push(read.sampleId) });
  context.after(() => reader.stop());
  await settle();
  assert.equal(requests[0].resetInterval, true);
  context.mock.timers.tick(2_000);
  await settle();
  assert.equal(requests[1].resetInterval, false);
  reader.stop();
  context.mock.timers.tick(20_000);
  await settle();
  assert.deepEqual(reads, [1, 2]);
  assert.equal(requests.length, 2);
});

test("selection replacement accepts only the latest requested details", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const first = deferred<ResourceRead>();
  const latest = deferred<ResourceRead>();
  const requests: ResourceReadRequest[] = [];
  let selection: ResourceSelection | null = null;
  const received: Array<ResourceSelection | null> = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => selection,
    read: (request) => { requests.push(request); return requests.length === 1 ? first.promise : latest.promise; },
    onLoading: () => {}, onError: () => {}, onRead: (_read, selected) => received.push(selected) });
  context.after(() => reader.stop());
  for (let page = 1; page <= 1000; page++) { selection = { group: "codex", page }; reader.refresh(); }
  first.resolve(reading(1));
  await settle();
  assert.equal(requests.length, 2);
  assert.deepEqual(requests[1].details, { group: "codex", page: 1000 });
  assert.deepEqual(received, []);
  latest.resolve(reading(2));
  await settle();
  assert.deepEqual(received, [{ group: "codex", page: 1000 }]);
});

test("hide cancels pending work and rejects a late native response", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const pending = deferred<ResourceRead>();
  let calls = 0;
  const received: ResourceRead[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: () => { calls++; return pending.promise; }, onLoading: () => {}, onError: () => {},
    onRead: (read) => received.push(read) });
  context.after(() => reader.stop());
  reader.refresh();
  reader.stop();
  pending.resolve(reading(1));
  await settle();
  context.mock.timers.tick(20_000);
  await settle();
  assert.equal(calls, 1);
  assert.deepEqual(received, []);
});

test("failed refresh retains the accepted reading and retries without resetting CPU", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const requests: ResourceReadRequest[] = [];
  const received: number[] = [];
  const errors: unknown[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async (request) => {
      requests.push(request);
      if (requests.length === 2) throw new Error("access restricted");
      return reading(requests.length);
    }, onLoading: () => {}, onError: (error) => { if (error) errors.push(error); },
    onRead: (read) => received.push(read.sampleId) });
  context.after(() => reader.stop());
  await settle();
  reader.refresh();
  await settle();
  assert.deepEqual(received, [1]);
  assert.equal(errors.length, 1);
  context.mock.timers.tick(5_000);
  await settle();
  assert.deepEqual(received, [1, 3]);
  assert.equal(requests[2].resetInterval, false);
});
