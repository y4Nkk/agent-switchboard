import assert from "node:assert/strict";
import { test } from "node:test";
import { catalogModel, providerPreset, providerPresets } from "../src/lib/provider-catalog.ts";
import { addCatalogModel, catalogEntry, withCatalog } from "../src/components/codex-provider-editor/catalog.ts";
import { applyCodexPreset, codexDraftFrom, prepareCodexDraft } from "../src/components/codex-provider-editor/draft.ts";
import { applyCodexDiscovery, reconcileCodexOperationDefaults, restoreCodexAutomaticOperation,
  setCodexOperation } from "../src/components/codex-provider-editor/operations.ts";

const modelId = "gpt-6.1-sol";
function draft(endpoint = "https://api.openai.com/v1") {
  return { ...codexDraftFrom(null), endpoint, name: "Fixture", apiKey: "isolated-fixture-key", parameters: { settings: {} } };
}

test("catalog matching is scoped to the complete API root, protocol and model ID", () => {
  assert.equal(providerPreset("https://api.openai.com/v1/")?.id, "openai");
  assert.equal(providerPreset("https://api.openai.com/v1/other-plan"), undefined);
  assert.equal(providerPreset("https://api.openai.com/v1?deployment=private"), undefined);
  assert.equal(catalogModel("https://relay.invalid/v1", "responses", modelId), undefined);
  assert.equal(catalogModel("https://api.openai.com/v1", "chatCompletions", modelId), undefined);
  assert.ok(catalogModel("https://api.openai.com/v1", "responses", modelId));
  assert.ok(providerPresets("codex").every(({ protocol }) => protocol !== "geminiGenerateContent"));
});

test("unknown providers retain missing limits instead of borrowing same-name model defaults", () => {
  const next = addCatalogModel(draft("https://relay.invalid/v1"), modelId);
  assert.equal(next.catalog[0].contextWindow, null);
  assert.equal(next.catalog[0].maxOutputTokens, null);
  assert.deepEqual(next.catalog[0].supportedReasoningLevels, ["none"]);
  assert.equal(next.catalog[0].functionTools, false);
  assert.equal(prepareCodexDraft(next), null);
  for (const invalid of [0, -1, 0.5, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER + 1]) {
    next.catalog[0].contextWindow = invalid;
    next.catalog[0].maxOutputTokens = 4096;
    assert.equal(prepareCodexDraft(next), null);
  }
});

test("preset changes keep user identity, credentials and model values while clearing incompatible Chat controls", () => {
  const original = addCatalogModel(draft(), modelId);
  original.upstream = "chatCompletions";
  original.capabilities.chatReasoning = { kind: "configured", thinkingParameter: "thinking",
    separateOutput: true, effortParameter: "reasoningEffort", effortMode: "passthrough" };
  const preset = providerPreset("https://api.anthropic.com")!;
  const next = applyCodexPreset(original, preset);
  assert.equal(next.name, "Fixture");
  assert.equal(next.apiKey, "isolated-fixture-key");
  assert.deepEqual(next.catalog, original.catalog);
  assert.equal(next.upstream, "anthropicMessages");
  assert.deepEqual(next.capabilities.chatReasoning, { kind: "unsupported" });
  assert.equal(original.capabilities.chatReasoning.kind, "configured");
});

test("choosing a known model creates its catalog and primary without a capability checklist", () => {
  const original = draft();
  const next = addCatalogModel(original, modelId);
  const known = catalogModel(original.endpoint, "responses", modelId)!;
  assert.equal(next.defaultModel, modelId);
  assert.equal(next.catalog[0].contextWindow, known.contextWindow);
  assert.equal(next.catalog[0].maxOutputTokens, known.maxOutputTokens);
  assert.deepEqual(next.catalog[0].supportedReasoningLevels, known.reasoningLevels);
  assert.equal(next.capabilities.functionTools, known.functionTools);
  assert.ok(prepareCodexDraft(next));
  assert.equal(original.catalog.length, 0);
});

test("adding an existing ID keeps explicit values and does not silently replace the primary", () => {
  const next = addCatalogModel(draft(), modelId);
  next.catalog[0].contextWindow = 42000;
  next.catalog[0].images = false;
  const again = addCatalogModel(next, modelId);
  assert.equal(again.catalog.length, 1);
  assert.equal(again.catalog[0].contextWindow, 42000);
  assert.equal(again.catalog[0].images, false);
  const additional = addCatalogModel(again, "manual-model");
  assert.equal(additional.defaultModel, modelId);
  assert.equal(additional.catalog[1].contextWindow, null);
});

test("model capabilities have one edit owner while independent provider operations are preserved", () => {
  const original = draft();
  original.capabilities.imageEdit = true;
  const entry = { ...catalogEntry(original, modelId), customTools: true, toolSearch: true };
  const enabled = withCatalog(original, [entry]);
  assert.equal(enabled.capabilities.customTools, true);
  assert.equal(enabled.capabilities.toolSearch, true);
  assert.equal(enabled.capabilities.imageEdit, true);
  const removed = withCatalog(enabled, []);
  assert.equal(removed.capabilities.customTools, false);
  assert.equal(removed.capabilities.reasoning, false);
  assert.equal(removed.capabilities.imageEdit, true);
  assert.deepEqual(original.catalog, []);
});

test("discovery updates only automatic operations with evidence from the current connection", () => {
  const original = codexDraftFrom(null);
  const chat = reconcileCodexOperationDefaults(original, { ...original, endpoint: "https://api.z.ai/api/paas/v4",
    upstream: "chatCompletions" });
  const manual = setCodexOperation(setCodexOperation(chat, "imageEdit", true), "alphaSearch", false);
  const detected = applyCodexDiscovery(manual, {
    models: [{ id: "glm-5.3-flash", ownedBy: null }], confirmedOperations: ["alphaSearch", "imageGeneration"],
  });
  assert.equal(detected.capabilities.models, true);
  assert.equal(detected.capabilities.chatCompletions, true);
  assert.equal(detected.capabilities.alphaSearch, false);
  assert.equal(detected.capabilities.imageGeneration, true);
  assert.equal(detected.capabilities.imageEdit, true);

  const changed = reconcileCodexOperationDefaults(detected, { ...detected, endpoint: "https://relay.invalid/v1", upstream: "responses" });
  assert.equal(changed.capabilities.models, false);
  assert.equal(changed.capabilities.chatCompletions, false);
  assert.equal(changed.capabilities.imageGeneration, false);
  assert.equal(changed.capabilities.imageEdit, true);
});

test("an empty automatic set keeps saved operations manual until an individual operation is restored", () => {
  const original = draft();
  const saved = { ...original, capabilities: { ...original.capabilities,
    automaticOperations: [], models: true, imageEdit: true } };
  assert.equal(applyCodexDiscovery(saved, null), saved);
  const restored = restoreCodexAutomaticOperation(saved, "models");
  assert.deepEqual(restored.capabilities.automaticOperations, ["models"]);
  assert.equal(restored.capabilities.models, false);
  assert.equal(restored.capabilities.imageEdit, true);
  const manualAgain = setCodexOperation(restored, "models", true);
  assert.deepEqual(manualAgain.capabilities.automaticOperations, []);
});
