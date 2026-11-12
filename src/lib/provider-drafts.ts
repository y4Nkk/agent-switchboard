import type { ClaudeModelSettings, CodexProviderDraft, CodexProviderRecord, ModelOptions, ProviderDraft, ProviderRecord } from "../api/client";

/** Copies stored facts without applying editor defaults or changing secrets. */
export function codexProviderDraft(record: CodexProviderRecord): CodexProviderDraft {
  const { id: _id, routeMode: _routeMode, ...profile } = record.profile;
  return {
    ...profile,
    catalog: profile.catalog.map((entry) => ({ ...entry, supportedReasoningLevels: [...entry.supportedReasoningLevels] })),
    modelRoutes: profile.modelRoutes.map((route) => ({ ...route })),
    subagentRoute: profile.subagentRoute ? { ...profile.subagentRoute } : null,
    capabilities: { ...profile.capabilities },
    parameters: { settings: { ...record.parameters.settings } },
    notes: record.notes ?? null,
    websiteUrl: record.websiteUrl ?? null,
    usageQuery: record.usageQuery ?? null,
  };
}

export function providerDraft(record: ProviderRecord): ProviderDraft {
  const { id: _id, ...profile } = record.profile;
  return { ...profile, parameters: { settings: { ...profile.parameters.settings } } };
}

export function claudeOptions(current: ModelOptions | null, patch: Partial<ClaudeModelSettings>): ModelOptions {
  const base: ClaudeModelSettings = current?.kind === "claude" ? current : {
    primaryOneM: false, availableModels: null,
    routes: { haiku: null, sonnet: null, opus: null, fable: null, subagent: null },
  };
  return { kind: "claude", ...base, ...patch };
}
