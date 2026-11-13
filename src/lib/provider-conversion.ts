import type { CodexConversionDraft, ProviderDraft, UsageQuery } from "../api/client";
import type { CodexEditorDraft } from "../components/codex-provider-editor/draft";

export function providerDraftFromConversion(seed: ProviderDraft): ProviderDraft {
  const copied = structuredClone(seed);
  return { ...copied, responsesOptions: copied.responsesOptions ?? null,
    modelOptions: copied.modelOptions ?? null, connection: copied.connection ?? {},
    claudeFragment: copied.claudeFragment ?? {}, usageQuery: copied.usageQuery ?? null };
}

/** Conversion keeps unknown limits and source flags intact until reviewed. */
export function codexDraftFromConversion(seed: CodexConversionDraft): CodexEditorDraft {
  const copied = structuredClone(seed);
  return { ...copied, authentication: copied.authentication ?? null,
    connection: copied.connection ?? {}, subagentRoute: copied.subagentRoute ?? null,
    notes: copied.notes ?? "", websiteUrl: copied.websiteUrl ?? "" };
}

export function conversionLimitsComplete(draft: Pick<CodexEditorDraft, "catalog">): boolean {
  return draft.catalog.length > 0 && draft.catalog.every((entry) =>
    Number.isSafeInteger(entry.contextWindow) && (entry.contextWindow ?? 0) > 0
    && Number.isSafeInteger(entry.maxOutputTokens) && (entry.maxOutputTokens ?? 0) > 0);
}

export function conversionInputsComplete(draft: Pick<CodexEditorDraft, "catalog">, capabilitiesReviewed: boolean): boolean {
  return capabilitiesReviewed && conversionLimitsComplete(draft);
}

export function selectedConversionUsage(query: UsageQuery | null, selected: boolean): UsageQuery | null {
  return selected && query ? { ...structuredClone(query), refreshIntervalMinutes: 0 } : null;
}
