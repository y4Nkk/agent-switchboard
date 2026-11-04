import assert from "node:assert/strict";
import { test } from "node:test";
import { startModelUsageReading, type ModelUsageActivity } from "../src/pages/model-usage-reader.ts";
import { modelUsageProgress } from "../src/pages/model-usage-progress.ts";
import type { ModelUsageRead, ModelUsageRefresh } from "../src/api/model-usage.ts";

function reading(): ModelUsageRead {
  return { freshness: "cached", refreshAfter: new Date(Date.now() + 300_000).toISOString(),
    reports: (["today", "last7Days", "last30Days", "all"] as const).map((range) => ({
      range, generatedAt: new Date().toISOString(), groups: [], days: [], issues: [],
      unassignedTokens: { inputTokens: 0, cacheReadInputTokens: 0, cacheCreationInputTokens: 0, outputTokens: 0, totalTokens: 0 },
      coverage: { sourceSessions: 0, scannedFiles: 0, sessionsWithoutUsage: 0, undatedEventTokens: 0 },
    })) };
}
function running(): Extract<ModelUsageRefresh, { status: "running" }> {
  return { status: "running", id: "one", startedAt: new Date().toISOString(), progress: {
    phase: "reading", completedSources: 1, totalSources: 3, completedBytes: 250,
    totalBytes: 1_000, completedCalls: 0, totalCalls: null,
  } };
}
async function settle() { for (let step = 0; step < 8; step++) await Promise.resolve(); }

test("saved ranges appear before the backend task finishes, and repeated refreshes join", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const reads: ModelUsageRead[] = [];
  const activity: ModelUsageActivity[] = [];
  const progress: unknown[] = [];
  const saved = reading();
  let starts = 0;
  let polls = 0;
  const reader = startModelUsageReading({ readSnapshot: async () => saved,
    refresh: async () => { starts++; return running(); },
    readRefresh: async () => { polls++; return { status: "completed", id: "one", startedAt: running().startedAt, read: { ...saved, freshness: "fresh" } }; },
    onRead: (read) => reads.push(read), onProgress: (task) => progress.push(task),
    onActivity: (value) => activity.push(value), onError: () => {} });
  context.after(() => reader.stop());
  await settle();
  assert.deepEqual(reads, [saved]);
  for (const range of ["all", "today", "last7Days", "last30Days"]) assert.ok(reads[0].reports.find((report) => report.range === range));
  await reader.refresh();
  assert.equal(starts, 1);
  assert.equal(polls, 0);
  assert.equal((progress.at(-1) as ReturnType<typeof running>).progress.completedBytes, 250);
  context.mock.timers.tick(250);
  await settle();
  assert.equal(reads.at(-1)?.freshness, "fresh");
  assert.equal(activity.at(-1), null);
});

test("hiding releases observation and ignores late task responses", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  let complete!: (task: ModelUsageRefresh) => void;
  const pending = new Promise<ModelUsageRefresh>((resolve) => { complete = resolve; });
  const reads: ModelUsageRead[] = [];
  let polls = 0;
  const reader = startModelUsageReading({ readSnapshot: async () => null, refresh: async () => running(),
    readRefresh: () => { polls++; return pending; }, onRead: (read) => reads.push(read),
    onProgress: () => {}, onActivity: () => {}, onError: () => {} });
  await settle();
  context.mock.timers.tick(250);
  await settle();
  reader.stop();
  complete({ status: "completed", id: "one", startedAt: running().startedAt, read: reading() });
  await settle();
  context.mock.timers.tick(300_000);
  assert.equal(polls, 1);
  assert.deepEqual(reads, []);
});

test("task failure preserves the snapshot and retries after thirty seconds", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const reads: ModelUsageRead[] = [];
  const errors: unknown[] = [];
  const saved = reading();
  let starts = 0;
  const reader = startModelUsageReading({ readSnapshot: async () => saved,
    refresh: async () => { starts++; return { status: "failed", id: "failed", startedAt: running().startedAt, message: "fixture failure" }; },
    readRefresh: async () => running(), onRead: (read) => reads.push(read), onProgress: () => {},
    onActivity: () => {}, onError: (error) => errors.push(error) });
  context.after(() => reader.stop());
  await settle();
  assert.deepEqual(reads, [saved]);
  assert.equal((errors.at(-1) as Error).message, "fixture failure");
  context.mock.timers.tick(29_999);
  assert.equal(starts, 1);
  context.mock.timers.tick(1);
  await settle();
  assert.equal(starts, 2);
});

test("repeated observation errors release loading and allow a new attempt", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const activities: ModelUsageActivity[] = [];
  let starts = 0;
  const reader = startModelUsageReading({ readSnapshot: async () => { throw new Error("bad snapshot"); },
    refresh: async () => { starts++; return running(); }, readRefresh: async () => { throw new Error("lost task"); },
    onRead: () => {}, onProgress: () => {}, onActivity: (activity) => activities.push(activity), onError: () => {} });
  context.after(() => reader.stop());
  await settle();
  for (const interval of [250, 1_000, 1_000]) { context.mock.timers.tick(interval); await settle(); }
  assert.equal(activities.at(-1), null);
  await reader.refresh();
  assert.equal(starts, 2);
});

test("progress is measured work and remains indeterminate without a known total", () => {
  assert.equal(modelUsageProgress(undefined), null);
  const progress = running().progress;
  assert.deepEqual(modelUsageProgress(progress), { unit: "bytes", done: 250, total: 1_000, percent: 25 });
  assert.equal(modelUsageProgress({ ...progress, phase: "discovering" }), null);
  assert.equal(modelUsageProgress({ ...progress, phase: "publishing", completedSources: 3 }), null);
  assert.equal(modelUsageProgress({ ...progress, totalBytes: null, totalSources: null }), null);
  assert.equal(modelUsageProgress({ ...progress, totalBytes: null, totalCalls: 8, completedCalls: 2 })?.percent, 25);
});
