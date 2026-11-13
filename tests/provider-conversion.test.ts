import assert from "node:assert/strict";
import { test } from "node:test";
import type { CodexConversionDraft } from "../src/api/provider-conversion.ts";
import { codexDraftFromConversion, conversionInputsComplete, conversionLimitsComplete, providerDraftFromConversion, selectedConversionUsage } from "../src/lib/provider-conversion.ts";

function seed(): CodexConversionDraft {
  return {
    name: "Source", endpoint: "https://example.invalid", apiKey: "fixture-key",
    upstream: "anthropicMessages", requestMode: "standard", defaultModel: "vendor-model",
    catalog: [{ id: "vendor-model", contextWindow: null, maxOutputTokens: null,
      functionTools: false, customTools: false, toolSearch: false, reasoning: false,
      defaultReasoningLevel: "none", supportedReasoningLevels: ["none"], images: false, compact: false }],
    modelRoutes: [], capabilities: { responses: true, compact: false, models: false,
      chatCompletions: false, alphaSearch: false, imageGeneration: false, imageEdit: false,
      functionTools: false, customTools: false, toolSearch: false, reasoning: false, chatReasoning: { kind: "unsupported" } },
    parameters: { settings: {} }, notes: null, websiteUrl: null, usageQuery: null,
  };
}

test("conversion seed keeps unknown limits and explicit false capabilities without editor defaults", () => {
  const original = seed();
  const draft = codexDraftFromConversion(original);
  assert.equal(draft.catalog[0].contextWindow, null);
  assert.equal(draft.catalog[0].maxOutputTokens, null);
  assert.equal(draft.catalog[0].images, false);
  assert.equal(draft.capabilities.functionTools, false);
  assert.equal(conversionInputsComplete(draft, true), false);
  draft.catalog[0].supportedReasoningLevels.push("high");
  draft.capabilities.functionTools = true;
  assert.deepEqual(original.catalog[0].supportedReasoningLevels, ["none"]);
  assert.equal(original.capabilities.functionTools, false);
});

test("conversion requires both explicit positive limits and reviewed capabilities", () => {
  const draft = codexDraftFromConversion(seed());
  for (const invalid of [null, 0, -1, 0.5, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER + 1]) {
    draft.catalog[0].contextWindow = invalid;
    draft.catalog[0].maxOutputTokens = 4096;
    assert.equal(conversionInputsComplete(draft, true), false);
    draft.catalog[0].contextWindow = 128000;
    draft.catalog[0].maxOutputTokens = invalid;
    assert.equal(conversionInputsComplete(draft, true), false);
  }
  draft.catalog[0].contextWindow = 128000;
  draft.catalog[0].maxOutputTokens = 4096;
  assert.equal(conversionLimitsComplete(draft), true);
  assert.equal(conversionInputsComplete(draft, false), false);
  assert.equal(conversionInputsComplete(draft, true), true);
  draft.catalog.push({ ...draft.catalog[0], id: "second", maxOutputTokens: null });
  assert.equal(conversionInputsComplete(draft, true), false);
  assert.equal(conversionLimitsComplete({ catalog: [] }), false);
});

test("usage queries require opt-in and copy with manual cadence without mutating the source", () => {
  const query = { kind: "script" as const, source: "({ request() {}, extract() {} })", refreshIntervalMinutes: 10 };
  assert.equal(selectedConversionUsage(query, false), null);
  assert.equal(selectedConversionUsage(null, true), null);
  const copied = selectedConversionUsage(query, true);
  assert.deepEqual(copied, { ...query, refreshIntervalMinutes: 0 });
  assert.equal(query.refreshIntervalMinutes, 10);
  assert.notEqual(copied, query);
});

test("Claude conversion normalizes omitted nullable Rust fields without changing protocol or source", () => {
  const source = { app: "claude", routeMode: "custom", upstreamProtocol: "anthropicMessages",
    name: "Converted", model: "vendor-model", baseUrl: "https://example.invalid", apiKey: "fixture-key",
    maxOutputTokens: null, parameters: { settings: {} }, websiteUrl: null };
  const draft = providerDraftFromConversion(source as Parameters<typeof providerDraftFromConversion>[0]);
  assert.equal(draft.responsesOptions, null);
  assert.equal(draft.modelOptions, null);
  assert.equal(draft.upstreamProtocol, "anthropicMessages");
  assert.deepEqual(draft.connection, {});
  assert.equal(draft.usageQuery, null);
  assert.equal("responsesOptions" in source, false);
});
