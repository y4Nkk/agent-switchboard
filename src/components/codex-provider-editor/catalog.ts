import type { CodexCapabilities, CodexChatReasoning, CodexReasoningLevel } from "../../api/client";
import { catalogModel, providerPreset } from "../../lib/provider-catalog.ts";
import type { CodexEditorDraft, EditableCodexCatalogEntry } from "./draft";

export function catalogEntry(draft: CodexEditorDraft, id: string): EditableCodexCatalogEntry {
  const model = catalogModel(draft.endpoint, draft.upstream, id);
  const chatConfigured = draft.capabilities.chatReasoning.kind === "configured"
    || !draft.catalog.length && presetChatReasoning(draft.endpoint).kind === "configured";
  const reasoning = !!model?.reasoning && model.reasoningLevels.length > 0
    && (draft.upstream !== "chatCompletions" || chatConfigured);
  const levels: CodexReasoningLevel[] = reasoning ? [...model!.reasoningLevels] : ["none"];
  return {
    id, contextWindow: model?.contextWindow ?? null, maxOutputTokens: model?.maxOutputTokens ?? null,
    functionTools: model?.functionTools ?? false, customTools: false, toolSearch: false,
    reasoning, images: model?.images ?? false, compact: false,
    supportedReasoningLevels: levels,
    defaultReasoningLevel: levels.includes("medium") ? "medium" : levels[0],
  };
}

/** Shared model flags have exactly one edit owner; the provider carries their aggregate. */
export function withCatalog(draft: CodexEditorDraft, catalog: EditableCodexCatalogEntry[]): CodexEditorDraft {
  const capabilities = { ...draft.capabilities };
  for (const key of ["functionTools", "customTools", "toolSearch", "compact", "reasoning"] as const) {
    capabilities[key] = catalog.some((entry) => entry[key]);
  }
  if (!capabilities.reasoning) capabilities.chatReasoning = { kind: "unsupported" };
  return { ...draft, catalog, capabilities };
}

export function addCatalogModel(draft: CodexEditorDraft, id: string): CodexEditorDraft {
  const model = id.trim();
  if (!model) return draft;
  const catalog = draft.catalog.some((entry) => entry.id === model)
    ? draft.catalog : [...draft.catalog, catalogEntry(draft, model)];
  const next = withCatalog({ ...draft, defaultModel: draft.defaultModel || model }, catalog);
  if (!draft.catalog.length && next.upstream === "chatCompletions" && next.capabilities.reasoning
    && next.capabilities.chatReasoning.kind === "unsupported") {
    next.capabilities.chatReasoning = presetChatReasoning(next.endpoint);
  }
  return next;
}

export function modelSource(draft: CodexEditorDraft, entry: EditableCodexCatalogEntry): "catalogSource" | "savedSource" {
  const known = catalogEntry(draft, entry.id);
  return catalogModel(draft.endpoint, draft.upstream, entry.id)
    && Object.keys(known).every((key) => JSON.stringify(known[key as keyof typeof known]) === JSON.stringify(entry[key as keyof typeof entry]))
    ? "catalogSource" : "savedSource";
}

export const emptyCapabilities = (): CodexCapabilities => ({
  responses: true, compact: false, models: false, chatCompletions: false,
  alphaSearch: false, imageGeneration: false, imageEdit: false,
  functionTools: false, customTools: false, toolSearch: false, reasoning: false,
  chatReasoning: { kind: "unsupported" },
});

export function presetChatReasoning(endpoint: string): CodexChatReasoning {
  const id = providerPreset(endpoint)?.id;
  if (id === "openrouter") return { kind: "configured", thinkingParameter: "none", separateOutput: false,
    effortParameter: "reasoningObject", effortMode: "openRouter" };
  if (id === "deepseek") return { kind: "configured", thinkingParameter: "thinking", separateOutput: false,
    effortParameter: "reasoningEffort", effortMode: "deepSeek" };
  return { kind: "unsupported" };
}
