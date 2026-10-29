import assert from "node:assert/strict";
import { test } from "node:test";
import type { CodexProviderRecord, ProviderRecord } from "../src/api/client.ts";
import { codexProviderDraft } from "../src/lib/provider-drafts.ts";
import { resetRouting, roleTarget, routeDocument, routeIssue, routeRecords, routeRoles } from "../src/components/model-routing/model.ts";

function codex(id: string): CodexProviderRecord {
  return { profile: { id, name: id, routeMode: "gateway", endpoint: "https://fixture.invalid", apiKey: "fixture-key",
    upstream: "responses", requestMode: "standard", defaultModel: "shared-model", connection: {},
    catalog: [{ id: "shared-model", images: false, supportedReasoningLevels: [] }], modelRoutes: [], capabilities: {}, subagentRoute: null },
    parameters: { settings: {} }, notes: null, websiteUrl: null, usageQuery: null, fileHash: id, position: 0 } as CodexProviderRecord;
}

function claude(): ProviderRecord {
  return { profile: { id: "claude", app: "claude", name: "Claude fixture", routeMode: "custom", model: "main-model",
    apiKey: "fixture-key", baseUrl: "https://fixture.invalid", parameters: { settings: {} },
    modelOptions: { kind: "claude", primaryOneM: true, sonnetModel: "tier-model", sonnetOneM: true,
      haikuModel: null, opusModel: null, opusOneM: false, availableModels: ["tier-model"] },
  }, fileHash: "claude", position: 0 } as ProviderRecord;
}

test("same model names remain owned by the explicitly chosen provider", () => {
  const [source, a, b] = [codex("source"), codex("a"), codex("b")];
  const document = routeDocument({ kind: "codex", record: source });
  if (document.kind !== "codex") throw new Error("wrong boundary");
  document.draft.subagentRoute = { profileId: "b", model: "shared-model" };
  assert.deepEqual(roleTarget(document, "subagent", [a, b]), { provider: "b", model: "shared-model" });
  assert.equal(routeIssue(document, [a, b]), null);
  assert.equal(routeIssue(document, [a]), "codex.subagent.missingTarget");
});

test("missing selections, bound targets and removed models cannot silently change route", () => {
  const source = codex("source");
  const target = codex("target");
  const document = routeDocument({ kind: "codex", record: source });
  if (document.kind !== "codex") throw new Error("wrong boundary");
  document.draft.subagentRoute = { profileId: "", model: "" };
  assert.equal(routeIssue(document, [target]), "modelRoutes.chooseTarget");
  document.draft.subagentRoute = { profileId: "target", model: "removed" };
  assert.equal(routeIssue(document, [target]), "codex.subagent.modelNotInCatalog");
  target.profile.connection = { authBinding: { kind: "fixture" } } as typeof target.profile.connection;
  assert.equal(routeIssue(document, [target]), "codex.subagent.authBoundTarget");
});

test("reset removes explicit subagent routing while preserving provider credentials and catalog", () => {
  const source = codex("source");
  const document = routeDocument({ kind: "codex", record: source });
  if (document.kind !== "codex") throw new Error("wrong boundary");
  document.draft.subagentRoute = { profileId: "target", model: "shared-model" };
  const reset = resetRouting(document);
  if (reset.kind !== "codex") throw new Error("wrong boundary");
  assert.equal(reset.draft.subagentRoute, null);
  assert.equal(reset.draft.apiKey, source.profile.apiKey);
  assert.deepEqual(reset.draft.catalog, source.profile.catalog);
  assert.ok(document.draft.subagentRoute);
});

test("client inventories and official routes expose only their supported roles", () => {
  const profile = claude();
  const inventory = { codex: [codex("codex")], profiles: [profile], activeIds: {} };
  assert.deepEqual(routeRecords(inventory, "codex").map(({ record }) => record.profile.id), ["codex"]);
  const document = routeDocument({ kind: "profile", record: profile });
  assert.deepEqual(routeRoles(document), ["main", "haiku", "sonnet", "opus", "fable"]);
  profile.profile.routeMode = "official";
  assert.deepEqual(routeRoles(routeDocument({ kind: "profile", record: profile })), ["main"]);
});

test("Claude reset clears tier overrides and context flags, retaining the saved model catalog", () => {
  const document = resetRouting(routeDocument({ kind: "profile", record: claude() }));
  if (document.kind !== "profile" || document.draft.modelOptions?.kind !== "claude") throw new Error("wrong boundary");
  assert.equal(document.draft.model, null);
  assert.equal(document.draft.modelOptions.sonnetModel, null);
  assert.equal(document.draft.modelOptions.sonnetOneM, false);
  assert.equal(document.draft.modelOptions.fableModel, null);
  assert.equal(document.draft.modelOptions.fableOneM, false);
  assert.equal(document.draft.modelOptions.primaryOneM, false);
  assert.deepEqual(document.draft.modelOptions.availableModels, ["tier-model"]);
});

test("Codex route drafts preserve stored capabilities and omit projection-only fields", () => {
  const record = codex("codex");
  const draft = codexProviderDraft(record);
  assert.equal(draft.catalog[0].images, false);
  assert.equal("id" in draft, false);
  assert.equal("routeMode" in draft, false);
  assert.notEqual(draft.catalog, record.profile.catalog);
  assert.notEqual(draft.parameters.settings, record.parameters.settings);
});
