import { invoke } from "./client";
import type { AppKind, AuthenticationScheme, CodexTransportClass, ModelOptions, ResponsesOptions, UpstreamProtocol } from "./shared";
import type { UsageQuery } from "./usage";
import type { FilePreview } from "./switching";
import type { SettingsValues } from "./settings";

export interface ProviderEndpoint {
  url: string;
  addedAt: number;
  lastUsed?: number | null;
}

export interface ProviderEndpointsView {
  providerId: string;
  fileHash: string;
  endpoints: ProviderEndpoint[];
}

export interface LocalProxyRequestOverrides {
  headers: Record<string, string>;
  body: Record<string, unknown> | null;
}

export interface ClaudeNativeConfiguration { kind: "bedrock" | "vertex" | "foundry"; environment: Record<string, string> }
export interface ClaudeBilling {
  costMultiplier: string;
  modelSource: "response" | "manual";
  dailyLimitUsd: string | null;
  monthlyLimitUsd: string | null;
}
export interface ProviderConnectionOptions {
  claudeNative?: ClaudeNativeConfiguration | null;
  codex?: import("./codex-request-options").CodexRequestOptions | null;
  isFullUrl?: boolean;
  customEndpoints?: Record<string, ProviderEndpoint>;
  endpointAutoSelect?: boolean | null;
  customUserAgent?: string | null;
  localProxyRequestOverrides?: LocalProxyRequestOverrides | null;
  authBinding?: { source: string; authProvider?: string | null; accountId?: string | null } | null;
  providerType?: string | null;
  apiKeyField?: "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY" | null;
  claudeBilling?: ClaudeBilling | null;
  claudePromptCacheKey?: string | null;
  modelsUrl?: string | null;
}

/** Source display columns; application-side metadata that never reaches any
 * client configuration and never participates in routing identity. */
export interface ProviderDisplay {
  icon?: string | null;
  iconColor?: string | null;
  category?: string | null;
  createdAt?: number | null;
}

export interface ProviderProfile {
  id: string;
  app: AppKind;
  routeMode: "official" | "custom";
  name: string;
  model: string | null;
  baseUrl: string | null;
  connection?: ProviderConnectionOptions | null;
  apiKey: string;
  authentication?: AuthenticationScheme | null;
  upstreamProtocol: UpstreamProtocol | null;
  responsesOptions: ResponsesOptions | null;
  maxOutputTokens: number | null;
  modelOptions: ModelOptions | null;
  parameters: SettingsValues;
  /** Claude-only additional settings.json fragment owned by this profile;
   * applied while active and removed on switch-away. Codex never sets it. */
  claudeFragment?: Record<string, unknown>;
  /** Local-only note; never written into any client configuration. */
  notes?: string | null;
  /** Provider homepage, used for navigation only. */
  websiteUrl: string | null;
  /** Source display columns; see ProviderDisplay. */
  display?: ProviderDisplay | null;
  /** Application-side usage-balance query; never written into client config. */
  usageQuery?: UsageQuery | null;
  /** Whole minutes between backend official Codex quota queries; absent or
   * null disables automatic queries. Never written into client config. */
  officialQuotaRefreshIntervalMinutes?: number | null;
}

/** One provider file together with its storage revision. `fileHash` guards
 * every mutation of that provider file against an external change. */
export interface ProviderRecord {
  profile: ProviderProfile;
  position: number;
  fileHash: string;
}

/** Routing is explicit. Official profiles retain no endpoint, API key, model
 * override, or usage script because client credentials stay native. */
export interface ProviderDraft {
  app: AppKind;
  routeMode: "official" | "custom";
  name: string;
  model: string | null;
  baseUrl: string | null;
  connection?: ProviderConnectionOptions | null;
  apiKey: string;
  authentication?: AuthenticationScheme | null;
  upstreamProtocol: UpstreamProtocol | null;
  responsesOptions: ResponsesOptions | null;
  maxOutputTokens: number | null;
  modelOptions: ModelOptions | null;
  parameters: SettingsValues;
  /** Claude-only additional settings.json fragment; see ProviderProfile. */
  claudeFragment?: Record<string, unknown>;
  notes?: string | null;
  websiteUrl: string | null;
  display?: ProviderDisplay | null;
  usageQuery?: UsageQuery | null;
  officialQuotaRefreshIntervalMinutes?: number | null;
}

export type CodexAuthenticationScheme = "bearer" | "xApiKey";
export type CodexUpstream = "responses" | "chatCompletions" | "anthropicMessages";
export type CodexRequestMode = ResponsesOptions["requestMode"];

export type CodexChatReasoning =
  | { kind: "unsupported" }
  | {
    kind: "configured";
    thinkingParameter: "none" | "thinking" | "enableThinking";
    separateOutput: boolean;
    effortParameter: "none" | "reasoningEffort" | "reasoningObject";
    effortMode: "passthrough" | "lowHigh" | "deepSeek" | "openRouter" | "catalog";
  };

export interface CodexCapabilities {
  responses: boolean;
  compact: boolean;
  models: boolean;
  chatCompletions: boolean;
  alphaSearch: boolean;
  imageGeneration: boolean;
  imageEdit: boolean;
  functionTools: boolean;
  customTools: boolean;
  toolSearch: boolean;
  reasoning: boolean;
  chatReasoning: CodexChatReasoning;
}

export interface CodexCatalogEntry {
  id: string;
  contextWindow: number;
  maxOutputTokens: number;
  functionTools: boolean;
  customTools: boolean;
  toolSearch: boolean;
  reasoning: boolean;
  defaultReasoningLevel: CodexReasoningLevel;
  supportedReasoningLevels: CodexReasoningLevel[];
  images: boolean;
  compact: boolean;
  displayName?: string | null;
  description?: string | null;
  baseInstructions?: string | null;
  supportsParallelToolCalls?: boolean | null;
}

export type CodexReasoningLevel = "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra";

export interface CodexModelRoute {
  clientModel: string;
  upstreamModel: string;
}

/** The current, complete third-party Codex contract. Official login is not
 * represented by a profile. */
export interface CodexProviderProfile {
  id: string;
  name: string;
  endpoint: string;
  apiKey: string;
  authentication?: CodexAuthenticationScheme | null;
  connection?: ProviderConnectionOptions | null;
  upstream: CodexUpstream;
  routeMode: "direct" | "gateway";
  requestMode: CodexRequestMode;
  defaultModel: string;
  catalog: CodexCatalogEntry[];
  modelRoutes: CodexModelRoute[];
  subagentRoute?: CodexSubagentRoute | null;
  capabilities: CodexCapabilities;
}

/** The sub-agent model reference: a cross-profile route resolved by the
 * local gateway. This is the only form the subagent model takes. */
export interface CodexSubagentRoute {
  profileId: string;
  model: string;
}

export interface CodexProviderDraft {
  name: string;
  endpoint: string;
  apiKey: string;
  authentication?: CodexAuthenticationScheme | null;
  connection?: ProviderConnectionOptions | null;
  upstream: CodexUpstream;
  requestMode: CodexRequestMode;
  defaultModel: string;
  catalog: CodexCatalogEntry[];
  modelRoutes: CodexModelRoute[];
  subagentRoute?: CodexSubagentRoute | null;
  capabilities: CodexCapabilities;
  parameters: SettingsValues;
  notes: string | null;
  websiteUrl: string | null;
  usageQuery: UsageQuery | null;
}

export interface CodexProviderRecord {
  profile: CodexProviderProfile;
  position: number;
  parameters: SettingsValues;
  notes: string | null;
  websiteUrl: string | null;
  usageQuery: UsageQuery | null;
  fileHash: string;
}

/** Reachability grade of one manual probe: any HTTP answer counts as ok/slow,
 * only network-level failures (DNS / refused / TLS / timeout) are unreachable. */
export type ProbeGrade = "ok" | "slow" | "unreachable";

export interface ProbeResult {
  grade: ProbeGrade;
  status: number | null;
  latencyMs: number | null;
  /** Set exactly when grade is unreachable; the text lives in the catalog. */
  errorClass: CodexTransportClass | null;
  at: string;
}

export type ProfileSaveKind = "create" | "noChange" | "saveOnly" | "saveAndApply";
export type ProfileSaveIntent = "save" | "activate";

/** Backend-owned classification of an edit against the stored profile and
 * the current client configuration. A candidate exists only for an edit that
 * must re-apply the active provider. */
export interface ProfileSavePreparation {
  preparationId: string;
  kind: ProfileSaveKind;
  preview: FilePreview | null;
}

export function listProfiles(): Promise<ProviderRecord[]> {
  return invoke<ProviderRecord[]>("list_profiles");
}

export function listCodexProfiles(): Promise<CodexProviderRecord[]> {
  return invoke<CodexProviderRecord[]>("list_codex_profiles");
}

export function createCodexProfile(draft: CodexProviderDraft): Promise<CodexProviderRecord> {
  return invoke<CodexProviderRecord>("create_codex_profile", { draft });
}

/** Prepares an existing Codex profile save. A live route change returns the
 * exact client-file preview that must be confirmed before it can be applied. */
export function prepareCodexProfileSave(
  profileId: string,
  draft: CodexProviderDraft,
  expectedFileHash: string,
  intent: ProfileSaveIntent,
): Promise<ProfileSavePreparation> {
  return invoke<ProfileSavePreparation>("prepare_codex_profile_save", {
    profileId,
    draft,
    expectedFileHash,
    intent,
  });
}

export function commitCodexProfileSave(
  preparationId: string,
  confirmWrite: boolean,
): Promise<CodexProviderRecord> {
  return invoke<CodexProviderRecord>("commit_codex_profile_save", {
    preparationId,
    confirmWrite,
  });
}

export function deleteCodexProfile(profileId: string, expectedFileHash: string): Promise<void> {
  return invoke<void>("delete_codex_profile", { profileId, expectedFileHash });
}

export function reorderCodexProfiles(
  orderedIds: string[],
  expectedFileHashes: Record<string, string>,
): Promise<void> {
  return invoke<void>("reorder_codex_profiles", {
    orderedIds,
    expectedFileHashes,
  });
}

export function prepareProfileSave(
  profileId: string | null,
  draft: ProviderDraft,
  expectedFileHash: string | null,
  intent: ProfileSaveIntent,
): Promise<ProfileSavePreparation> {
  return invoke<ProfileSavePreparation>("prepare_profile_save", { profileId, draft, expectedFileHash, intent });
}

export function commitProfileSave(
  preparationId: string,
  confirmWrite: boolean,
): Promise<ProviderRecord> {
  return invoke<ProviderRecord>("commit_profile_save", { preparationId, confirmWrite });
}

export type ProfileSaveSource = "profile" | "codex";

export function cancelProfileSave(source: ProfileSaveSource, preparationId: string): Promise<void> {
  return invoke<void>("cancel_profile_save", { source, preparationId });
}

export function deleteProfile(profileId: string, expectedFileHash: string): Promise<void> {
  return invoke<void>("delete_profile", { profileId, expectedFileHash });
}

export function reorderClaudeProfiles(
  orderedIds: string[],
  expectedFileHashes: Record<string, string>,
): Promise<void> {
  return invoke<void>("reorder_claude_profiles", {
    orderedIds,
    expectedFileHashes,
  });
}

export function importDiscoveredClaudeProfile(): Promise<ProviderRecord> {
  return invoke<ProviderRecord>("import_discovered_claude_profile");
}

export function listProviderEndpoints(providerId: string): Promise<ProviderEndpointsView> {
  return invoke<ProviderEndpointsView>("list_provider_endpoints", { providerId });
}

export function addProviderEndpoint(
  providerId: string,
  url: string,
  expectedFileHash: string,
  confirmWrite: boolean,
): Promise<ProviderEndpointsView> {
  return invoke<ProviderEndpointsView>("add_provider_endpoint", {
    providerId,
    url,
    expectedFileHash,
    confirmWrite,
  });
}

export function removeProviderEndpoint(
  providerId: string,
  url: string,
  expectedFileHash: string,
  confirmWrite: boolean,
): Promise<ProviderEndpointsView> {
  return invoke<ProviderEndpointsView>("remove_provider_endpoint", {
    providerId,
    url,
    expectedFileHash,
    confirmWrite,
  });
}

export function probeEndpoint(url: string): Promise<ProbeResult> {
  return invoke<ProbeResult>("probe_endpoint", { url });
}

export interface ProviderEndpoints {
  requestUrl: string;
  modelsUrl: string | null;
  modelsError: string | null;
}

/** Pure backend resolution, without contacting the provider or reading credentials. */
export function resolveProviderEndpoints(
  baseUrl: string,
  upstreamProtocol: UpstreamProtocol,
  connection?: ProviderConnectionOptions | null,
): Promise<ProviderEndpoints> {
  return invoke("resolve_provider_endpoints", {
    request: { baseUrl, upstreamProtocol, ...(connection ? { connection } : {}) },
  });
}

/** One model from the provider's configured models endpoint. */
export interface ProviderModel {
  id: string;
  ownedBy: string | null;
}
/** Models from the provider's configured API root. */
export function fetchProviderModels(
  app: AppKind,
  baseUrl: string,
  apiKey: string,
  upstreamProtocol: UpstreamProtocol,
  authentication?: AuthenticationScheme | null,
  connection?: ProviderConnectionOptions | null,
): Promise<ProviderModel[]> {
  return invoke<ProviderModel[]>("fetch_provider_models", {
    request: {
      app,
      url: baseUrl,
      apiKey,
      upstreamProtocol,
      ...(authentication ? { authentication } : {}),
      ...(connection ? { connection } : {}),
    },
  });
}
