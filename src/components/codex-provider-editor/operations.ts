import type { CodexAutomaticOperation, CodexProbedOperation, ProviderModelDiscovery } from "../../api/client";
import { providerPreset } from "../../lib/provider-catalog.ts";
import type { CodexEditorDraft } from "./draft";

export const AUTOMATIC_OPERATIONS = [
  "models", "chatCompletions", "alphaSearch", "imageGeneration", "imageEdit",
] as const satisfies readonly CodexAutomaticOperation[];

export function isProbedOperation(operation: CodexAutomaticOperation): operation is CodexProbedOperation {
  return operation !== "models" && operation !== "chatCompletions";
}

function chatCompatibility(draft: CodexEditorDraft): boolean {
  if (draft.upstream === "anthropicMessages") return false;
  let endpoint = draft.endpoint.trim();
  if (!endpoint) return false;
  if (draft.connection.isFullUrl) {
    try {
      const url = new URL(endpoint);
      const path = url.pathname.replace(/\/+$/, "");
      const operation = /\/(chat\/completions|responses(?:\/compact)?)$/i.exec(path);
      if (!operation) return false;
      url.pathname = path.slice(0, operation.index);
      endpoint = url.href;
    } catch { return false; }
  }
  return draft.upstream === "chatCompletions"
    || !!providerPreset(endpoint, "chatCompletions")
    || providerPreset(endpoint, "responses")?.id === "openai";
}

export function reconcileCodexOperationDefaults(previous: CodexEditorDraft, draft: CodexEditorDraft): CodexEditorDraft {
  return sameCodexDiscoveryConnection(previous, draft) ? draft : applyCodexDiscovery(draft, null);
}

export function sameCodexDiscoveryConnection(left: CodexEditorDraft, right: CodexEditorDraft): boolean {
  return left.endpoint.trim() === right.endpoint.trim() && left.upstream === right.upstream
    && left.apiKey === right.apiKey && left.authentication === right.authentication
    && left.connection === right.connection;
}

export function applyCodexDiscovery(draft: CodexEditorDraft, result: ProviderModelDiscovery | null): CodexEditorDraft {
  const values = { ...draft.capabilities };
  const confirmed = new Set(result?.confirmedOperations);
  for (const operation of values.automaticOperations) {
    if (operation === "models") values.models = Boolean(result?.models.length);
    else if (operation === "chatCompletions") values.chatCompletions = chatCompatibility(draft);
    else values[operation] = confirmed.has(operation);
  }
  return AUTOMATIC_OPERATIONS.some((operation) => values[operation] !== draft.capabilities[operation])
    ? { ...draft, capabilities: values } : draft;
}

export function setCodexOperation(draft: CodexEditorDraft, operation: CodexAutomaticOperation, value: boolean): CodexEditorDraft {
  const automaticOperations = draft.capabilities.automaticOperations.filter((item) => item !== operation);
  return { ...draft, capabilities: { ...draft.capabilities, [operation]: value, automaticOperations } };
}

export function restoreCodexAutomaticOperation(draft: CodexEditorDraft, operation: CodexAutomaticOperation): CodexEditorDraft {
  const automaticOperations = AUTOMATIC_OPERATIONS.filter((item) => item === operation
    || draft.capabilities.automaticOperations.includes(item));
  return { ...draft, capabilities: { ...draft.capabilities, automaticOperations,
    [operation]: operation === "chatCompletions" && chatCompatibility(draft) } };
}
