import assert from "node:assert/strict";
import test from "node:test";
import { quotaProviders, refreshProviderQuotas, type QuotaProvider } from "../src/lib/quota-profiles.ts";

const query = { kind: "declarative" as const, url: "https://quota.example.test", remainingPath: "/remaining", refreshIntervalMinutes: 0 };

test("quota sources project both current provider stores without leaking credential fields", () => {
  const sources = quotaProviders([
    { profile: { app: "claude", routeMode: "custom", id: "claude", name: "Claude source", usageQuery: query } },
    { profile: { app: "claude", routeMode: "official", id: "native", name: "Native login", usageQuery: null } },
  ], [{ profile: { id: "codex", name: "Codex source" }, usageQuery: query }]);
  assert.deepEqual(sources.map(({ app, id, usageQuery }) => [app, id, usageQuery !== null]),
    [["codex", "codex", true], ["claude", "claude", true], ["claude", "native", false]]);
  assert.deepEqual(Object.keys(sources[0]).sort(), ["app", "id", "name", "usageQuery"]);
});

test("quota refresh is bounded, skips unconfigured sources, and continues after individual failures", async () => {
  const profiles: QuotaProvider[] = Array.from({ length: 7 }, (_, index) => ({
    app: index % 2 === 0 ? "codex" : "claude", id: String(index), name: String(index), usageQuery: index === 6 ? null : query,
  }));
  let active = 0;
  let maximum = 0;
  const calls: string[] = [];
  await refreshProviderQuotas(profiles, async (app, id) => {
    active += 1; maximum = Math.max(maximum, active); calls.push(`${app}:${id}`);
    await new Promise((resolve) => setTimeout(resolve, 1));
    active -= 1;
    if (id === "1" || id === "4") throw new Error("fixture query failure");
  });
  assert.equal(maximum, 3);
  assert.equal(calls.length, 6);
  assert.equal(new Set(calls).size, 6);
  assert.ok(!calls.includes("codex:6"));
});

test("an empty quota inventory makes no requests", async () => {
  let called = false;
  await refreshProviderQuotas([], async () => { called = true; });
  assert.equal(called, false);
});
