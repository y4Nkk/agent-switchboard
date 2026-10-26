import type {
  AppKind,
  ProviderDraft,
  ProviderProfile,
  SettingsValues,
  UpstreamProtocol,
} from "../../api/client";
import type { MessageKey } from "../../i18n";
import { NATIVE_PROTOCOL } from "../../lib/protocol";
import { normalizeUsageQuery } from "../../lib/usage-query";

export type ProviderEditorDraft = Omit<ProviderDraft, "parameters"> & {
  parameters: SettingsValues | null;
};

/** What each wire format means for the endpoint the user is about to enter. */
export const PROTOCOL_NOTES: Record<UpstreamProtocol, MessageKey> = {
  responses: "providers.protocolNote.responses",
  chatCompletions: "providers.protocolNote.chatCompletions",
  anthropicMessages: "providers.protocolNote.anthropicMessages",
  geminiGenerateContent: "providers.protocolNote.geminiGenerateContent",
};

export const PROTOCOL_AUTHENTICATION_NOTES: Record<UpstreamProtocol, MessageKey> = {
  geminiGenerateContent: "providers.protocolAuth.geminiGenerateContent",
  responses: "providers.protocolAuth.responses",
  chatCompletions: "providers.protocolAuth.chatCompletions",
  anthropicMessages: "providers.protocolAuth.anthropicMessages",
};

export function defaultConnection(app: AppKind): Pick<ProviderDraft,
  "upstreamProtocol" | "maxOutputTokens" | "responsesOptions" | "connection"> {
  return {
    upstreamProtocol: NATIVE_PROTOCOL[app],
    maxOutputTokens: null,
    responsesOptions: null,
    connection: {},
  };
}

export function draftFrom(profile: ProviderProfile | null, initialApp: AppKind): ProviderEditorDraft {
  if (profile) {
    return {
      app: profile.app,
      routeMode: profile.routeMode,
      name: profile.name,
      model: profile.model,
      baseUrl: profile.baseUrl,
      connection: profile.connection ? { ...profile.connection } : {},
      apiKey: profile.apiKey,
      authentication: profile.authentication ?? null,
      upstreamProtocol: profile.upstreamProtocol,
      responsesOptions: profile.responsesOptions ?? null,
      maxOutputTokens: profile.maxOutputTokens,
      modelOptions: profile.modelOptions ?? null,
      parameters: { settings: { ...profile.parameters.settings } },
      claudeFragment: profile.claudeFragment ? { ...profile.claudeFragment } : {},
      notes: profile.notes ?? null,
      websiteUrl: profile.websiteUrl,
      display: profile.display ?? null,
      usageQuery: profile.usageQuery ?? null,
      officialQuotaRefreshIntervalMinutes:
        profile.officialQuotaRefreshIntervalMinutes ?? null,
    };
  }
  return {
    app: initialApp,
    routeMode: "custom",
    name: "",
    model: null,
    baseUrl: null,
    apiKey: "",
    ...defaultConnection(initialApp),
    modelOptions: null,
    parameters: null,
    claudeFragment: {},
    notes: null,
    websiteUrl: null,
    display: null,
    usageQuery: null,
    officialQuotaRefreshIntervalMinutes: null,
  };
}

export function prepareDraft(draft: ProviderEditorDraft): ProviderDraft | null {
  if (!draft.parameters || !responsesOptionsValid(draft)) return null;
  const fragment = draft.claudeFragment ?? {};
  return {
    ...draft,
    parameters: draft.parameters,
    name: draft.name.trim(),
    model: optional(draft.model ?? ""),
    baseUrl: optional(draft.baseUrl ?? ""),
    apiKey: draft.apiKey.trim(),
    notes: optional(draft.notes ?? ""),
    websiteUrl: optional(draft.websiteUrl ?? ""),
    modelOptions: draft.modelOptions,
    usageQuery: normalizeUsageQuery(draft.usageQuery),
    // Match the backend wire shape: an absent fragment is the empty state.
    claudeFragment: Object.keys(fragment).length ? fragment : undefined,
  };
}

export function responsesOptionsValid(draft: ProviderEditorDraft): boolean {
  if (draft.routeMode !== "custom" || draft.upstreamProtocol !== "responses") {
    return draft.responsesOptions === null;
  }
  const options = draft.responsesOptions;
  return Boolean(options && Object.keys(options).length === 1 && (
    options.requestMode === "standard" || options.requestMode === "minimal"
  ));
}

export function optional(value: string): string | null {
  const normalized = value.trim();
  return normalized || null;
}
