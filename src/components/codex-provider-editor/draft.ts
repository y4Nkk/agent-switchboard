import { reconcileCodexRequestOptions } from "../../api/codex-request-options.ts";
import type {
  CodexCapabilities,
  CodexCatalogEntry,
  CodexModelRoute,
  CodexProviderDraft,
  CodexProviderRecord,
  CodexReasoningLevel,
  CodexRequestMode,
  CodexSubagentRoute,
  CodexUpstream,
  ProviderConnectionOptions,
  CodexAuthenticationScheme,
  SettingsValues,
  UsageQuery,
} from "../../api/client";
import type { ProviderPreset } from "../../lib/provider-catalog.ts";
import { emptyCapabilities } from "./catalog.ts";
import { normalizeUsageQuery } from "../../lib/usage-query.ts";
import type { MessageKey, TFunction } from "../../i18n";

/** The editable shape of one third-party Codex profile. `parameters` stays
 * null until the parameter catalog has seeded it, exactly like the Claude
 * editor draft. */
export interface CodexEditorDraft {
  name: string;
  endpoint: string;
  apiKey: string;
  authentication: CodexAuthenticationScheme | null;
  connection: ProviderConnectionOptions;
  upstream: CodexUpstream;
  requestMode: CodexRequestMode;
  defaultModel: string;
  catalog: EditableCodexCatalogEntry[];
  modelRoutes: CodexModelRoute[];
  subagentRoute: CodexSubagentRoute | null;
  capabilities: CodexCapabilities;
  parameters: SettingsValues | null;
  notes: string;
  websiteUrl: string;
  usageQuery: UsageQuery | null;
}

/** Missing limits remain explicit until a known catalog value or the user supplies them. */
export type EditableCodexCatalogEntry = Omit<CodexCatalogEntry, "contextWindow" | "maxOutputTokens"> & {
  contextWindow: number | null;
  maxOutputTokens: number | null;
};

export const REASONING_LEVELS: readonly CodexReasoningLevel[] = ["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

export const REASONING_LEVEL_LABELS: Record<CodexReasoningLevel, MessageKey> = {
  none: "codex.level.none",
  minimal: "codex.level.minimal",
  low: "codex.level.low",
  medium: "codex.level.medium",
  high: "codex.level.high",
  xhigh: "codex.level.xhigh",
  max: "codex.level.max",
  ultra: "codex.level.ultra",
};

export function codexDraftFrom(record: CodexProviderRecord | null): CodexEditorDraft {
  if (!record) {
    return {
      name: "",
      endpoint: "",
      apiKey: "",
      authentication: null,
      connection: {},
      upstream: "responses",
      requestMode: "standard",
      defaultModel: "",
      catalog: [],
      modelRoutes: [],
      subagentRoute: null,
      capabilities: emptyCapabilities(),
      parameters: null,
      notes: "",
      websiteUrl: "",
      usageQuery: null,
    };
  }
  return {
    name: record.profile.name,
    endpoint: record.profile.endpoint,
    apiKey: record.profile.apiKey,
    authentication: record.profile.authentication ?? null,
    connection: record.profile.connection ? { ...record.profile.connection } : {},
    upstream: record.profile.upstream,
    requestMode: record.profile.requestMode,
    defaultModel: record.profile.defaultModel,
    catalog: record.profile.catalog.map((entry) => ({ ...entry, supportedReasoningLevels: [...entry.supportedReasoningLevels] })),
    modelRoutes: record.profile.modelRoutes.map((route) => ({ ...route })),
    subagentRoute: record.profile.subagentRoute
      ? { ...record.profile.subagentRoute }
      : null,
    capabilities: { ...record.profile.capabilities },
    parameters: { settings: { ...record.parameters.settings } },
    notes: record.notes ?? "",
    websiteUrl: record.websiteUrl ?? "",
    usageQuery: record.usageQuery ?? null,
  };
}

export function prepareCodexDraft(draft: CodexEditorDraft): CodexProviderDraft | null {
  if (!draft.parameters || !draft.catalog.length || draft.catalog.some((entry) => !validLimits(entry))) return null;
  return {
    name: draft.name.trim(),
    endpoint: draft.endpoint.trim(),
    apiKey: draft.apiKey.trim(),
    authentication: draft.authentication,
    connection: draft.connection,
    upstream: draft.upstream,
    requestMode: draft.requestMode,
    defaultModel: draft.defaultModel.trim(),
    catalog: draft.catalog.map(prepareCatalogEntry),
    modelRoutes: draft.modelRoutes,
    subagentRoute: draft.subagentRoute ? { ...draft.subagentRoute } : null,
    capabilities: draft.capabilities,
    parameters: draft.parameters,
    notes: optional(draft.notes),
    websiteUrl: optional(draft.websiteUrl),
    usageQuery: normalizeUsageQuery(draft.usageQuery),
  };
}

function validLimits(entry: EditableCodexCatalogEntry): boolean {
  return [entry.contextWindow, entry.maxOutputTokens].every((value) => Number.isSafeInteger(value) && (value ?? 0) > 0);
}

function prepareCatalogEntry({
  contextWindow,
  maxOutputTokens,
  ...entry
}: EditableCodexCatalogEntry): CodexCatalogEntry {
  return {
    ...entry,
    contextWindow: contextWindow!,
    maxOutputTokens: maxOutputTokens!,
  };
}

function optional(value: string): string | null {
  const normalized = value.trim();
  return normalized || null;
}

/** Client-side mirror of `contracts::codex::validate`. The backend stays the
 * authority; this only blocks obviously invalid saves with readable reasons. */
export function validateCodexDraft(draft: CodexEditorDraft, t: TFunction): string[] {
  const problems: string[] = [];
  if (!draft.name.trim()) problems.push(t("codex.validate.nameRequired"));
  if (!draft.apiKey.trim()) problems.push(t("codex.validate.apiKeyRequired"));
  const endpoint = draft.endpoint.trim();
  if (!endpoint) {
    problems.push(t("codex.validate.endpointRequired"));
  } else {
    try {
      const parsed = new URL(endpoint);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
        problems.push(t("codex.validate.endpointProtocol"));
      }
    } catch {
      problems.push(t("codex.validate.endpointInvalid"));
    }
  }
  const { capabilities } = draft;
  if (!capabilities.responses) problems.push(t("codex.validate.responsesRequired"));
  const chatReasoningConfigured = capabilities.chatReasoning.kind === "configured";
  if (!capabilities.reasoning && chatReasoningConfigured) {
    problems.push(t("codex.validate.chatReasoningNeedsReasoning"));
  }
  if (draft.upstream !== "chatCompletions" && chatReasoningConfigured) {
    problems.push(t("codex.validate.chatReasoningUpstream"));
  }
  if (draft.catalog.length === 0) {
    problems.push(t("codex.validate.catalogEmpty"));
  } else {
    const ids = new Set<string>();
    for (const entry of draft.catalog) {
      if (!entry.id.trim()) problems.push(t("codex.validate.modelIdRequired"));
      if (ids.has(entry.id)) problems.push(t("codex.validate.modelDuplicate", { id: entry.id }));
      ids.add(entry.id);
      if (entry.functionTools && !capabilities.functionTools) problems.push(t("codex.validate.functionToolsExceed", { id: entry.id }));
      if (entry.customTools && !capabilities.customTools) problems.push(t("codex.validate.customToolsExceed", { id: entry.id }));
      if (entry.toolSearch && !capabilities.toolSearch) problems.push(t("codex.validate.toolSearchExceed", { id: entry.id }));
      if (entry.compact && !capabilities.compact) problems.push(t("codex.validate.compactExceed", { id: entry.id }));
      if (entry.reasoning && !capabilities.reasoning) problems.push(t("codex.validate.reasoningExceed", { id: entry.id }));
      if (!validLimits(entry)) {
        problems.push(t("providerSetup.limitsRequired", { model: entry.id }));
      }
      if (entry.supportedReasoningLevels.some((level) => !REASONING_LEVELS.includes(level))) {
        problems.push(t("providerSetup.invalidLevels", { model: entry.id }));
      }
      const levels = new Set(entry.supportedReasoningLevels);
      if (entry.supportedReasoningLevels.length === 0) problems.push(t("codex.validate.levelsMissing", { id: entry.id }));
      if (levels.size !== entry.supportedReasoningLevels.length) problems.push(t("codex.validate.levelsDuplicate", { id: entry.id }));
      if (!levels.has(entry.defaultReasoningLevel)) problems.push(t("codex.validate.defaultLevelMissing", { id: entry.id }));
      if (!entry.reasoning && (entry.defaultReasoningLevel !== "none"
        || entry.supportedReasoningLevels.length !== 1 || !levels.has("none"))) {
        problems.push(t("codex.validate.nonReasoningLevels", { id: entry.id }));
      }
    }
    if (!draft.defaultModel.trim()) problems.push(t("codex.validate.defaultModelRequired"));
    else if (!ids.has(draft.defaultModel)) problems.push(t("codex.validate.defaultModelMissing"));
  }
  const catalogIds = new Set(draft.catalog.map((entry) => entry.id));
  const routeSources = new Set<string>();
  for (const route of draft.modelRoutes) {
    if (!route.clientModel.trim() || !route.upstreamModel.trim()) problems.push(t("codex.validate.mappingEmpty"));
    if (!catalogIds.has(route.clientModel)) problems.push(t("codex.validate.mappingUnknownClient", { model: route.clientModel }));
    if (routeSources.has(route.clientModel)) problems.push(t("codex.validate.mappingDuplicate", { model: route.clientModel }));
    routeSources.add(route.clientModel);
  }
  const subagentRoute = draft.subagentRoute;
  if (subagentRoute) {
    if (!UUID_PATTERN.test(subagentRoute.profileId)) {
      problems.push(t("codex.validate.routeProfileInvalid"));
    }
    if (!subagentRoute.model.trim()) {
      problems.push(t("codex.validate.routeModelRequired"));
    }
  }
  return problems;
}

/** Hyphenated UUID shape, mirroring the backend's canonical profile-id check. */
const UUID_PATTERN = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

/** Chat reasoning may survive only on a Chat Completions upstream. */
export function reconcileCodexUpstream(draft: CodexEditorDraft, upstream: CodexUpstream): CodexEditorDraft {
  const capabilities = upstream === "chatCompletions"
    ? draft.capabilities
    : { ...draft.capabilities, chatReasoning: { kind: "unsupported" as const } };
  return { ...draft, upstream, capabilities, connection: reconcileCodexRequestOptions(draft.connection, upstream) };
}

export function applyCodexPreset(draft: CodexEditorDraft, preset: ProviderPreset): CodexEditorDraft {
  if (preset.protocol === "geminiGenerateContent") throw new Error("Gemini Native cannot configure Codex");
  const next = reconcileCodexUpstream(draft, preset.protocol);
  return { ...next, endpoint: preset.endpoint, name: draft.name || preset.name,
    websiteUrl: draft.websiteUrl || preset.website || "",
  };
}
