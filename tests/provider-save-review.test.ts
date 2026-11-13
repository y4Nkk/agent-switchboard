import assert from "node:assert/strict";
import { test } from "node:test";
import type { AppKind, ProviderRecord } from "../src/api/client.ts";
import { providerSaveReview } from "../src/app/provider-save-review.ts";
import { routeDocument, type RouteInventory } from "../src/components/model-routing/model.ts";

function official(app: AppKind, id: string): ProviderRecord {
  return { profile: { id, app, name: `${app} official`, routeMode: "official", model: null, baseUrl: null,
    apiKey: "", upstreamProtocol: null, responsesOptions: null, maxOutputTokens: null, modelOptions: null,
    parameters: { settings: {} }, websiteUrl: null }, fileHash: id, position: 1 };
}

test("Codex official saves report the Codex active identity from the generic store", () => {
  const codex = official("codex", "11111111-1111-4111-8111-111111111111");
  const claude = official("claude", "22222222-2222-4222-8222-222222222222");
  const inventory: RouteInventory = { codex: [], profiles: [codex, claude],
    activeIds: { codex: codex.profile.id, claude: claude.profile.id } };
  const document = routeDocument({ kind: "profile", record: codex });
  const review = providerSaveReview("codex", document, document, inventory);
  assert.equal(review.activeName, "codex official");
  assert.deepEqual(review.changes, []);
});

test("a purpose-only save describes its target without switching the active provider", () => {
  const source = official("claude", "11111111-1111-4111-8111-111111111111");
  source.profile.routeMode = "custom";
  source.profile.model = "main";
  const target = official("claude", "22222222-2222-4222-8222-222222222222");
  target.profile.name = "Target";
  const inventory: RouteInventory = { codex: [], profiles: [source, target], activeIds: { claude: source.profile.id } };
  const before = routeDocument({ kind: "profile", record: source });
  const after = { ...before, draft: { ...before.draft, modelOptions: { kind: "claude" as const,
    primaryOneM: false, availableModels: null,
    routes: { haiku: null, sonnet: null, opus: null, fable: null,
      subagent: { mode: "provider" as const, profileId: target.profile.id, model: "fast", oneM: true } } } } };
  const review = providerSaveReview("claude", before, after, inventory);
  assert.equal(review.activeName, source.profile.name);
  assert.deepEqual(review.changes.map(({ role }) => role), ["subagent"]);
  assert.deepEqual(review.changes[0].after.target, { provider: "Target", model: "fast" });
  assert.equal(review.changes[0].after.oneM, true);
  assert.equal(inventory.activeIds.claude, source.profile.id);
});
