import assert from "node:assert/strict";
import { test } from "node:test";
import type { CodexProviderRecord, ProviderRecord } from "../src/api/client.ts";
import { codexProviderDraft } from "../src/lib/provider-drafts.ts";
import { resetRouting, roleTarget, routeDocument, routeIssue, routeRecords, routeRoles, routeDescription, routeIssueRole, type RouteDocument } from "../src/components/model-routing/model.ts";

function codex(id: string): CodexProviderRecord {
  return { profile: { id, name: id, routeMode: "gateway", endpoint: "https://fixture.invalid", apiKey: "fixture-key",
    upstream: "responses", requestMode: "standard", defaultModel: "shared-model", connection: {},
    catalog: [{ id: "shared-model", images: false, supportedReasoningLevels: [] }], modelRoutes: [], capabilities: {}, subagentRoute: null },
    parameters: { settings: {} }, notes: null, websiteUrl: null, usageQuery: null, fileHash: id, position: 0 } as CodexProviderRecord;
}

function inventory(codex: CodexProviderRecord[] = [], profiles: ProviderRecord[] = []) {
  return { codex, profiles, activeIds: {} };
}

function claude(): ProviderRecord {
  return { profile: { id: "claude", app: "claude", name: "Claude fixture", routeMode: "custom", model: "main-model",
    apiKey: "fixture-key", baseUrl: "https://fixture.invalid", parameters: { settings: {} },
    connection: {}, modelOptions: { kind: "claude", primaryOneM: true,
      routes: { haiku: null, sonnet: { mode: "primary", model: "tier-model", oneM: true }, opus: null, fable: null, subagent: null },
      availableModels: ["tier-model"] },
  }, fileHash: "claude", position: 0 } as ProviderRecord;
}

test("same model names remain owned by the explicitly chosen provider", () => {
  const [source, a, b] = [codex("source"), codex("a"), codex("b")];
  const document = routeDocument({ kind: "codex", record: source });
  if (document.kind !== "codex") throw new Error("wrong boundary");
  document.draft.subagentRoute = { profileId: "b", model: "shared-model" };
  assert.deepEqual(roleTarget(document, "subagent", inventory([a, b])), { provider: "b", model: "shared-model" });
  assert.equal(routeIssue(document, inventory([a, b])), null);
  assert.equal(routeIssue(document, inventory([a])), "codex.subagent.missingTarget");
});

test("missing selections, bound targets and removed models cannot silently change route", () => {
  const source = codex("source");
  const target = codex("target");
  const document = routeDocument({ kind: "codex", record: source });
  if (document.kind !== "codex") throw new Error("wrong boundary");
  document.draft.subagentRoute = { profileId: "", model: "" };
  assert.equal(routeIssue(document, inventory([target])), "modelRoutes.chooseTarget");
  document.draft.subagentRoute = { profileId: "target", model: "removed" };
  assert.equal(routeIssue(document, inventory([target])), "codex.subagent.modelNotInCatalog");
  target.profile.connection = { authBinding: { kind: "fixture" } } as typeof target.profile.connection;
  assert.equal(routeIssue(document, inventory([target])), "codex.subagent.authBoundTarget");
});

test("reset removes explicit subagent routing while preserving the main model and catalog", () => {
  const source = codex("source");
  const document = routeDocument({ kind: "codex", record: source });
  if (document.kind !== "codex") throw new Error("wrong boundary");
  document.draft.subagentRoute = { profileId: "target", model: "shared-model" };
  const reset = resetRouting(document);
  if (reset.kind !== "codex") throw new Error("wrong boundary");
  assert.equal(reset.draft.subagentRoute, null);
  assert.equal(reset.draft.defaultModel, source.profile.defaultModel);
  assert.deepEqual(reset.draft.catalog, source.profile.catalog.map(({ id }) => ({ id })));
  assert.equal("apiKey" in reset.draft, false);
  assert.ok(document.draft.subagentRoute);
});

test("client inventories and official routes expose only their supported roles", () => {
  const profile = claude();
  const inventory = { codex: [codex("codex")], profiles: [profile], activeIds: {} };
  assert.deepEqual(routeRecords(inventory, "codex").map(({ record }) => record.profile.id), ["codex"]);
  const document = routeDocument({ kind: "profile", record: profile });
  assert.deepEqual(routeRoles(document), ["main", "haiku", "sonnet", "opus", "fable", "subagent"]);
  profile.profile.routeMode = "official";
  assert.deepEqual(routeRoles(routeDocument({ kind: "profile", record: profile })), ["main"]);
});

test("Claude reset clears purpose overrides while retaining the main model, its context flag and catalog", () => {
  const document = resetRouting(routeDocument({ kind: "profile", record: claude() }));
  if (document.kind !== "profile" || document.draft.modelOptions?.kind !== "claude") throw new Error("wrong boundary");
  assert.equal(document.draft.model, "main-model");
  assert.deepEqual(document.draft.modelOptions.routes, { haiku: null, sonnet: null, opus: null, fable: null, subagent: null });
  assert.equal(document.draft.modelOptions.primaryOneM, true);
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

test("Claude cross-provider purpose identity is explicit and never inferred from matching model names", () => {
  const source = claude();
  const target = claude();
  target.profile.id = "target";
  target.profile.name = "Target provider";
  const document = routeDocument({ kind: "profile", record: source });
  if (document.kind !== "profile" || document.draft.modelOptions?.kind !== "claude") throw new Error("wrong boundary");
  document.draft.modelOptions = { ...document.draft.modelOptions, routes: { ...document.draft.modelOptions.routes,
    subagent: { mode: "provider", profileId: "target", model: "tier-model", oneM: false } } };
  assert.equal(routeIssue(document, inventory([], [source, target])), null);
  assert.deepEqual(roleTarget(document, "subagent", inventory([], [source, target])),
    { provider: "Target provider", model: "tier-model" });
  assert.equal(routeIssue(document, inventory([], [source])), "modelRoutes.missingTarget");
  target.profile.connection = { authBinding: { source: "fixture" } };
  assert.equal(routeIssue(document, inventory([], [source, target])), "modelRoutes.unsupportedTarget");
});

test("route summaries distinguish context flags and fixed targets, including self references", () => {
  const source = claude();
  const document = routeDocument({ kind: "profile", record: source });
  if (document.kind !== "profile" || document.draft.modelOptions?.kind !== "claude") throw new Error("wrong boundary");
  const options = document.draft.modelOptions;
  const before = routeDescription(document, "main", inventory([], [source]));
  document.draft.modelOptions = { ...options, primaryOneM: false,
    routes: { ...options.routes, sonnet: { mode: "provider", profileId: source.profile.id, model: "self-declared", oneM: true } } };
  assert.notDeepEqual(before, routeDescription(document, "main", inventory([], [source])));
  assert.equal(routeIssue(document, inventory([], [source])), null);
  assert.equal(routeDescription(document, "sonnet", inventory([], [source])).mode, "provider");
});


test("a new Codex self target uses the unsaved owner's model catalog without a stored record", () => {
  const profileId = "a7ae5414-5b71-459a-839e-f01f31ac2c8c";
  const document: RouteDocument = { kind: "codex", profileId, draft: {
    name: "Unsaved owner", defaultModel: "draft-model", catalog: [{ id: "draft-model" }],
    connection: {}, subagentRoute: { profileId, model: "draft-model" },
  } };
  assert.deepEqual(roleTarget(document, "subagent", inventory()), { provider: "Unsaved owner", model: "draft-model" });
  assert.equal(routeIssue(document, inventory()), null);
  document.draft.catalog = [{ id: "other" }];
  document.draft.defaultModel = "other";
  assert.equal(routeIssueRole(document, inventory()), "subagent");
  assert.equal(routeIssue(document, inventory()), "codex.subagent.modelNotInCatalog");
});

test("a new Claude fixed self target keeps its provider mode and unsaved display name", () => {
  const profileId = "a7ae5414-5b71-459a-839e-f01f31ac2c8c";
  const source = claude();
  const draft = { ...source.profile, name: "Unsaved Claude owner" };
  if (draft.modelOptions?.kind !== "claude") throw new Error("wrong boundary");
  draft.modelOptions = { ...draft.modelOptions, routes: { ...draft.modelOptions.routes,
    subagent: { mode: "provider", profileId, model: "tier-model", oneM: true } } };
  const document: RouteDocument = { kind: "profile", profileId, draft };
  assert.equal(routeIssue(document, inventory()), null);
  assert.deepEqual(routeDescription(document, "subagent", inventory()), {
    target: { provider: "Unsaved Claude owner", model: "tier-model" }, mode: "provider", oneM: true,
  });
});
