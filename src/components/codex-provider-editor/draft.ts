import { reconcileCodexRequestOptions } from "../../api/codex-request-options";
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
  ProviderModel,
  SettingsValues,
  UsageQuery,
} from "../../api/client";
import { normalizeUsageQuery } from "../../lib/usage-query";
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

/** One editor catalog row. The two limits may stay empty (null): empty means
 * "use the model's default" and is materialized with a concrete number only
 * when the draft is prepared for save. */
export type EditableCodexCatalogEntry = Omit<CodexCatalogEntry, "contextWindow" | "maxOutputTokens"> & {
  contextWindow: number | null;
  maxOutputTokens: number | null;
};

/** Explicit starting capability declaration; the catalog generator only ever
 * narrows entries against what the user declared here. */
export const DEFAULT_CODEX_CAPABILITIES: CodexCapabilities = {
  responses: true,
  compact: true,
  models: true,
  chatCompletions: false,
  alphaSearch: false,
  imageGeneration: false,
  imageEdit: false,
  functionTools: true,
  customTools: true,
  toolSearch: true,
  reasoning: true,
  chatReasoning: { kind: "unsupported" },
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

/** Generic positive fallbacks for models with neither a source-stated nor an
 * officially published limit; the backend only requires positive numbers. */
export const DEFAULT_CONTEXT_WINDOW = 128_000;
export const DEFAULT_MAX_OUTPUT_TOKENS = 8_192;

/** Official published model limits, cited per official model id (OpenAI model
 * pages, 2026-09). A source-stated fact always wins over this table; model
 * ids not listed here keep the generic editable defaults. */
export const OFFICIAL_MODEL_LIMITS: Record<string, { contextWindow: number; maxOutputTokens: number }> = {
  "gpt-6-astra": { contextWindow: 1_050_000, maxOutputTokens: 128_000 },
  "gpt-5.6-sol": { contextWindow: 1_050_000, maxOutputTokens: 128_000 },
  "gpt-5.6-terra": { contextWindow: 1_050_000, maxOutputTokens: 128_000 },
  "gpt-5.6-luna": { contextWindow: 1_050_000, maxOutputTokens: 128_000 },
};

/** The single owner of "leave empty" limit resolution: official published
 * limits for a known official id, the generic positive defaults otherwise. */
export function defaultModelLimits(modelId: string): { contextWindow: number; maxOutputTokens: number } {
  return OFFICIAL_MODEL_LIMITS[modelId]
    ?? { contextWindow: DEFAULT_CONTEXT_WINDOW, maxOutputTokens: DEFAULT_MAX_OUTPUT_TOKENS };
}

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
      capabilities: { ...DEFAULT_CODEX_CAPABILITIES },
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
    catalog: record.profile.catalog.map((entry) => ({ ...entry, images: true, supportedReasoningLevels: [...entry.supportedReasoningLevels] })),
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
  if (!draft.parameters) return null;
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

function prepareCatalogEntry({
  contextWindow,
  maxOutputTokens,
  ...entry
}: EditableCodexCatalogEntry): CodexCatalogEntry {
  const limits = defaultModelLimits(entry.id);
  return {
    ...entry,
    contextWindow: contextWindow ?? limits.contextWindow,
    maxOutputTokens: maxOutputTokens ?? limits.maxOutputTokens,
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

/** Source facts that override the generated catalog-row defaults. Absent
 * facts keep the editable defaults; the capability declaration itself is
 * never inferred. */
export interface CatalogSeedFacts {
  contextWindow?: number | null;
  defaultReasoningLevel?: CodexReasoningLevel | null;
  reasoningLevels?: readonly CodexReasoningLevel[] | null;
}

/** One editable catalog row for a fetched or seeded upstream model. Only the
 * row is generated from the declared capabilities plus explicit source facts;
 * the capability declaration itself is never inferred. Limits stay empty for
 * models without a source-stated or officially published value. */
export function catalogEntryFromModel(
  model: ProviderModel,
  capabilities: CodexCapabilities,
  facts: CatalogSeedFacts = {},
): EditableCodexCatalogEntry {
  const reasoning = capabilities.reasoning;
  const official = OFFICIAL_MODEL_LIMITS[model.id];
  const levels = reasoning
    ? [...new Set(facts.reasoningLevels && facts.reasoningLevels.length > 0
      ? facts.reasoningLevels
      : REASONING_LEVELS)]
    : ["none" as const];
  const fallbackDefault = levels.includes("medium") ? "medium" : levels[levels.length - 1];
  const defaultReasoningLevel = reasoning
    && facts.defaultReasoningLevel
    && levels.includes(facts.defaultReasoningLevel)
    ? facts.defaultReasoningLevel
    : reasoning ? fallbackDefault : "none";
  return {
    id: model.id,
    contextWindow: facts.contextWindow && facts.contextWindow > 0
      ? facts.contextWindow
      : official?.contextWindow ?? null,
    maxOutputTokens: official?.maxOutputTokens ?? null,
    functionTools: capabilities.functionTools,
    customTools: capabilities.customTools,
    toolSearch: capabilities.toolSearch,
    reasoning,
    defaultReasoningLevel,
    supportedReasoningLevels: levels,
    images: true,
    compact: capabilities.compact,
  };
}

/** A blank editable row for manual entry, narrowed to the declared capabilities. */
export function emptyCatalogEntry(capabilities: CodexCapabilities): EditableCodexCatalogEntry {
  return catalogEntryFromModel({ id: "", ownedBy: null }, capabilities);
}

/** Adds discovered models while keeping image input enabled by default. */
export function mergeFetchedCatalog(
  current: EditableCodexCatalogEntry[],
  models: ProviderModel[],
  capabilities: CodexCapabilities,
): EditableCodexCatalogEntry[] {
  const refreshed = [...current];
  const known = new Set(refreshed.map((entry) => entry.id));
  const added = models
    .filter((model) => !known.has(model.id))
    .map((model) => catalogEntryFromModel(model, capabilities));
  return [...refreshed, ...added];
}

/** Keeps catalog, routes, and chat reasoning consistent with the declared
 * capabilities and upstream: entries never exceed what is declared, and chat
 * reasoning exists only on Chat Completions with reasoning enabled. */
export function reconcileCodexCapabilities(
  draft: CodexEditorDraft,
  capabilities: CodexCapabilities,
): CodexEditorDraft {
  const nextCapabilities: CodexCapabilities =
    capabilities.reasoning && (capabilities.chatReasoning.kind === "configured")
      ? capabilities
      : { ...capabilities, chatReasoning: { kind: "unsupported" } };
  const catalog = draft.catalog.map((entry) => {
    const reasoning = entry.reasoning && nextCapabilities.reasoning;
    return {
      ...entry,
      functionTools: entry.functionTools && nextCapabilities.functionTools,
      customTools: entry.customTools && nextCapabilities.customTools,
      toolSearch: entry.toolSearch && nextCapabilities.toolSearch,
      compact: entry.compact && nextCapabilities.compact,
      reasoning,
      defaultReasoningLevel: reasoning ? entry.defaultReasoningLevel : "none",
      supportedReasoningLevels: reasoning ? entry.supportedReasoningLevels : ["none" as const],
    };
  });
  return { ...draft, capabilities: nextCapabilities, catalog };
}

/** Chat reasoning may survive only on a Chat Completions upstream. */
export function reconcileCodexUpstream(draft: CodexEditorDraft, upstream: CodexUpstream): CodexEditorDraft {
  const capabilities = upstream === "chatCompletions"
    ? draft.capabilities
    : { ...draft.capabilities, chatReasoning: { kind: "unsupported" as const } };
  return { ...draft, upstream, capabilities, connection: reconcileCodexRequestOptions(draft.connection, upstream) };
}
