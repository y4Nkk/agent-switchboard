import assert from "node:assert/strict";
import { test } from "node:test";
import { createReadQueue } from "../src/hooks/use-read-queue.ts";
import { startResourceReading, type ResourceReadActivity } from "../src/pages/resource-usage-reader.ts";
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
    onActivity: () => {}, onError: () => {}, onRead: (read) => reads.push(read.sampleId) });
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
  const activities: ResourceReadActivity[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => selection,
    read: (request) => { requests.push(request); return requests.length === 1 ? first.promise : latest.promise; },
    onActivity: (activity) => activities.push(activity), onError: () => {}, onRead: (_read, selected) => received.push(selected) });
  context.after(() => reader.stop());
  for (let page = 1; page <= 1000; page++) { selection = { group: "codex", page }; reader.select(); }
  first.resolve(reading(1));
  await settle();
  assert.equal(requests.length, 2);
  assert.deepEqual(requests[1].details, { group: "codex", page: 1000 });
  assert.deepEqual(requests.map((request) => request.resetInterval), [true, false]);
  assert.deepEqual(received, []);
  assert.deepEqual(activities, ["initial", "details"]);
  latest.resolve(reading(2));
  await settle();
  assert.deepEqual(received, [{ group: "codex", page: 1000 }]);
  assert.deepEqual(activities, ["initial", "details", null]);
});

test("hide cancels pending work and rejects a late native response", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const pending = deferred<ResourceRead>();
  let calls = 0;
  const received: ResourceRead[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: () => { calls++; return pending.promise; }, onActivity: () => {}, onError: () => {},
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
    }, onActivity: () => {}, onError: (error) => { if (error) errors.push(error); },
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

test("one minute of automatic samples publishes no foreground activity", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const activities: ResourceReadActivity[] = [];
  const requests: ResourceReadRequest[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async (request) => { requests.push(request); return reading(requests.length); },
    onActivity: (activity) => activities.push(activity), onError: () => {}, onRead: () => {} });
  context.after(() => reader.stop());
  await settle();
  for (let sample = 0; sample < 30; sample++) {
    context.mock.timers.tick(2_000);
    await settle();
  }
  assert.equal(requests.length, 31);
  assert.deepEqual(activities, ["initial", null]);
  assert.ok(requests.slice(1).every((request) => !request.resetInterval));
  reader.refresh();
  assert.deepEqual(activities, ["initial", null, "manual"]);
  await settle();
  assert.deepEqual(activities, ["initial", null, "manual", null]);
});

test("manual feedback survives a replaced response until the latest details finish", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const manual = deferred<ResourceRead>();
  const details = deferred<ResourceRead>();
  let selection: ResourceSelection | null = null;
  const activities: ResourceReadActivity[] = [];
  const received: number[] = [];
  const requests: ResourceReadRequest[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => selection,
    read: (request) => {
      requests.push(request);
      return requests.length === 1 ? Promise.resolve(reading(1)) : requests.length === 2 ? manual.promise : details.promise;
    }, onActivity: (activity) => activities.push(activity), onError: () => {},
    onRead: (read) => received.push(read.sampleId) });
  context.after(() => reader.stop());
  await settle();
  reader.refresh();
  selection = { group: "claude", page: 2 };
  reader.select();
  manual.resolve(reading(2));
  await settle();
  assert.deepEqual(received, [1]);
  assert.deepEqual(activities, ["initial", null, "manual"]);
  details.resolve({ ...reading(3), details: { group: "claude", page: 2, pageSize: 50, total: 51, rows: [] } });
  await settle();
  assert.deepEqual(received, [1, 3]);
  assert.deepEqual(activities, ["initial", null, "manual", null]);
  assert.deepEqual(requests.map((request) => request.resetInterval), [true, false, false]);
});

test("manual refresh replaces an in-flight automatic result without overlapping reads", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const automatic = deferred<ResourceRead>();
  const activities: ResourceReadActivity[] = [];
  const received: number[] = [];
  let calls = 0;
  let running = 0;
  let maximum = 0;
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async () => {
      calls++; running++; maximum = Math.max(maximum, running);
      try { return calls === 2 ? await automatic.promise : reading(calls); }
      finally { running--; }
    }, onActivity: (activity) => activities.push(activity), onError: () => {},
    onRead: (read) => received.push(read.sampleId) });
  context.after(() => reader.stop());
  await settle();
  context.mock.timers.tick(2_000);
  await settle();
  assert.deepEqual(activities, ["initial", null]);
  reader.refresh();
  assert.equal(calls, 2);
  assert.deepEqual(activities, ["initial", null, "manual"]);
  automatic.resolve(reading(2));
  await settle();
  assert.equal(maximum, 1);
  assert.deepEqual(received, [1, 3]);
  assert.deepEqual(activities, ["initial", null, "manual", null]);
});

test("a failed read keeps its diagnostic throughout automatic retry and clears on success", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const retry = deferred<ResourceRead>();
  const failure = new Error("read unavailable");
  const activities: ResourceReadActivity[] = [];
  const errors: unknown[] = [];
  const received: number[] = [];
  let calls = 0;
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async () => {
      calls++;
      if (calls === 2) throw failure;
      return calls === 3 ? retry.promise : reading(calls);
    }, onActivity: (activity) => activities.push(activity), onError: (error) => errors.push(error),
    onRead: (read) => received.push(read.sampleId) });
  context.after(() => reader.stop());
  await settle();
  context.mock.timers.tick(2_000);
  await settle();
  assert.deepEqual(errors, [null, failure]);
  context.mock.timers.tick(5_000);
  await settle();
  assert.deepEqual(errors, [null, failure]);
  assert.deepEqual(activities, ["initial", null]);
  assert.deepEqual(received, [1]);
  retry.resolve(reading(3));
  await settle();
  assert.deepEqual(errors, [null, failure, null]);
  assert.deepEqual(received, [1, 3]);
});

test("an initial failure preserves the reset requirement until a successful sample", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const requests: ResourceReadRequest[] = [];
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async (request) => {
      requests.push(request);
      if (requests.length === 1) throw new Error("collector unavailable");
      return reading(requests.length);
    }, onActivity: () => {}, onError: () => {}, onRead: () => {} });
  context.after(() => reader.stop());
  await settle();
  context.mock.timers.tick(5_000);
  await settle();
  context.mock.timers.tick(2_000);
  await settle();
  assert.deepEqual(requests.map((request) => request.resetInterval), [true, true, false]);
});

test("a hidden lifetime cannot publish into its resumed reader using the same queue", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const enqueue = createReadQueue();
  const hidden = deferred<ResourceRead>();
  const resumed = deferred<ResourceRead>();
  const requests: ResourceReadRequest[] = [];
  const received: number[] = [];
  const read = (request: ResourceReadRequest) => {
    requests.push(request);
    return requests.length === 1 ? hidden.promise : requests.length === 2 ? resumed.promise : Promise.resolve(reading(3));
  };
  const options = { enqueue, read, selection: () => null,
    onActivity: () => {}, onError: () => {}, onRead: (value: ResourceRead) => received.push(value.sampleId) };
  const beforeHide = startResourceReading(options);
  beforeHide.stop();
  const afterResume = startResourceReading(options);
  context.after(() => afterResume.stop());
  hidden.resolve(reading(1));
  await settle();
  assert.deepEqual(received, []);
  assert.deepEqual(requests.map((request) => request.resetInterval), [true, true]);
  resumed.resolve(reading(2));
  await settle();
  context.mock.timers.tick(2_000);
  await settle();
  assert.deepEqual(received, [2, 3]);
  assert.deepEqual(requests.map((request) => request.resetInterval), [true, true, false]);
});

function memoryReading(id: number, value: number | null): ResourceRead {
  const metric = (value: number | null): ResourceRead["groups"][number]["cpu"] => ({
    state: value === null ? "unavailable" : "measured", value,
    measuredProcesses: value === null ? 0 : 1, missingProcesses: value === null ? 1 : 0,
    issues: value === null ? ["accessDenied"] : [],
  });
  return { ...reading(id), groups: [{ group: "codex", presence: "running", issues: [],
    cpu: metric(0), residentBytes: metric(value), processCount: metric(1) }] };
}

test("memory scale grows within a visible lifetime, hides unknowns, and starts fresh after re-entry", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const values = [null, 0, 2 ** 30 - 1, 2 ** 30 + 1, 2 ** 30 - 1, null, 2 ** 30 - 1];
  const scales: Array<number | null> = [];
  const actual: Array<number | null> = [];
  let index = 0;
  const reader = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async () => memoryReading(index + 1, values[index++]),
    onActivity: () => {}, onError: () => {}, onRead: (read, _selected, scale) => {
      scales.push(scale); actual.push(read.groups[0].residentBytes.value);
    } });
  context.after(() => reader.stop());
  await settle();
  for (let sample = 1; sample < values.length; sample++) {
    context.mock.timers.tick(2_000);
    await settle();
  }
  assert.deepEqual(actual, values);
  assert.deepEqual(scales, [null, 2 ** 20, 2 ** 30, 2 ** 31, 2 ** 31, null, 2 ** 31]);
  reader.stop();
  const resumed = startResourceReading({ enqueue: createReadQueue(), selection: () => null,
    read: async () => memoryReading(8, 1), onActivity: () => {}, onError: () => {},
    onRead: (_read, _selection, scale) => scales.push(scale) });
  context.after(() => resumed.stop());
  await settle();
  assert.equal(scales.at(-1), 2 ** 20);
});
