import assert from "node:assert/strict";
import { test } from "node:test";
import { observeUpdateDownload } from "../src/app/update-download-reader.ts";
import { downloadPercent, latestDownloadState } from "../src/app/update-download-state.ts";
import type { UpdateDownloadState } from "../src/api/updates.ts";

function state(phase: UpdateDownloadState["phase"] = "downloading", revision = 0): UpdateDownloadState {
  return { id: "one", revision, version: "9.0.0", phase, downloadedBytes: 125, totalBytes: 1000, error: null };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
async function settle() { for (let index = 0; index < 8; index++) await Promise.resolve(); }

test("known byte totals yield bounded percentages and unknown totals stay unknown", () => {
  assert.equal(downloadPercent(state()), 12);
  assert.equal(downloadPercent({ ...state(), totalBytes: null }), null);
  assert.equal(downloadPercent({ ...state(), totalBytes: 0 }), null);
  assert.equal(downloadPercent({ ...state(), downloadedBytes: 1001 }), 100);
});

test("late polls cannot replace cancellation, terminal status or another task", () => {
  const cancelling = state("cancelling", 5);
  assert.equal(latestDownloadState(cancelling, state("downloading", 4)), cancelling);
  const cancelled = state("cancelled", 6);
  assert.equal(latestDownloadState(cancelled, cancelling), cancelled);
  assert.equal(latestDownloadState(cancelled, { ...state(), id: "old" }), cancelled);
  assert.equal(latestDownloadState(cancelled, { ...cancelled }), cancelled);
  assert.equal(latestDownloadState(cancelling, cancelled), cancelled);
});

test("slow reads never overlap, and the reader stops at a terminal status", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const first = deferred<UpdateDownloadState>();
  const received: UpdateDownloadState[] = [];
  let calls = 0;
  const stop = observeUpdateDownload("one", {
    read: async () => ++calls === 1 ? first.promise : state("cancelled", 2),
    onRead: (next) => received.push(next), onError: (error) => assert.equal(error, null),
  });
  context.after(stop);
  context.mock.timers.tick(10_000);
  await settle();
  assert.equal(calls, 1);
  first.resolve(state("downloading", 1));
  await settle();
  context.mock.timers.tick(199);
  assert.equal(calls, 1);
  context.mock.timers.tick(1);
  await settle();
  assert.equal(calls, 2);
  assert.equal(received.at(-1)?.phase, "cancelled");
  context.mock.timers.tick(10_000);
  await settle();
  assert.equal(calls, 2);
});

test("status-read failure preserves task identity and retries without reporting cancellation", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const received: UpdateDownloadState[] = [];
  const errors: unknown[] = [];
  let calls = 0;
  const stop = observeUpdateDownload("one", { read: async (id) => {
    assert.equal(id, "one");
    if (++calls === 1) throw new Error("fixture connection failed");
    return state("installed", 8);
  }, onRead: (next) => received.push(next), onError: (error) => errors.push(error) });
  context.after(stop);
  await settle();
  assert.equal(received.length, 0);
  assert.equal((errors[0] as Error).message, "fixture connection failed");
  context.mock.timers.tick(999);
  await settle();
  assert.equal(calls, 1);
  context.mock.timers.tick(1);
  await settle();
  assert.equal(received[0].phase, "installed");
  assert.equal(errors.at(-1), null);
});

test("stopping an old reader suppresses both late status and future requests", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const pending = deferred<UpdateDownloadState>();
  let calls = 0;
  const stop = observeUpdateDownload("old", { read: () => { calls++; return pending.promise; },
    onRead: () => assert.fail("obsolete reader published"), onError: () => assert.fail("obsolete reader reported") });
  stop();
  pending.resolve(state("installing", 5));
  await settle();
  context.mock.timers.tick(10_000);
  assert.equal(calls, 1);
});
