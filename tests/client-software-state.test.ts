import assert from "node:assert/strict";
import test from "node:test";
import type { ClientUpdateTask } from "../src/api/client-software.ts";
import { latestClientUpdate } from "../src/components/client-software/task-state.ts";

const task = (id: string, revision: number, phase: ClientUpdateTask["phase"]): ClientUpdateTask => ({
  id, revision, phase, startedAt: "2026-11-04T00:00:00Z", finishedAt: null, actualVersion: null,
  cancellable: phase === "preparing", error: null, log: [], processId: null, processBirth: null,
  plan: { id: "plan", installationId: "installation", app: "codex", launcher: "/fixture/codex",
    manager: "native", fromVersion: "0.165.0", targetVersion: "0.166.0", exactTarget: true,
    channel: "latest", proxyMode: "none", expiresAt: "2026-11-04T00:05:00Z" },
});

test("a late start or cancel response cannot replace a verified result", () => {
  const finished = task("one", 8, "completed");
  assert.equal(latestClientUpdate(finished, task("one", 1, "preparing")), finished);
  assert.equal(latestClientUpdate(finished, task("one", 7, "verifying")), finished);
  assert.equal(latestClientUpdate(finished, null), finished);
});

test("revisions remain ordered across separate operations", () => {
  const previous = task("one", 8, "failed");
  const next = task("two", 9, "preparing");
  assert.equal(latestClientUpdate(previous, next), next);
  assert.equal(latestClientUpdate(next, previous), next);
  assert.equal(latestClientUpdate(null, next), next);
});
