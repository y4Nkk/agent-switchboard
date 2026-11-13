import type { AppKind, CodexModelRoute, CodexSubagentRoute, ProviderDraft, UpstreamProtocol } from "../api/client";
import type { MessageKey, TFunction } from "../i18n";

/** TS mirror of the routing contract owned by asb-core
 * `UpstreamProtocol::native_for`: the wire protocol each client speaks
 * natively. */
export const NATIVE_PROTOCOL: Record<AppKind, UpstreamProtocol> = {
  codex: "responses",
  claude: "anthropicMessages",
};

/** A route converts wire protocols only when its client and upstream protocols differ. */
export function isProtocolTranslation(
  clientProtocol: UpstreamProtocol,
  upstreamProtocol: UpstreamProtocol,
): boolean {
  return clientProtocol !== upstreamProtocol;
}

/** Single owner of the UpstreamProtocol display label across pages and
 * forms. The wording lives in the i18n "protocol." domain so it follows the
 * interface language instead of staying English-only. */
const PROTOCOL_LABEL_KEYS: Record<UpstreamProtocol, MessageKey> = {
  responses: "protocol.responses",
  chatCompletions: "protocol.chatCompletions",
  anthropicMessages: "protocol.anthropicMessages",
  geminiGenerateContent: "protocol.geminiGenerateContent",
};

export function protocolLabel(t: TFunction, protocol: UpstreamProtocol): string {
  return t(PROTOCOL_LABEL_KEYS[protocol]);
}

/** Mirrors asb-core `ProviderConnectionOptions::normalize_endpoint_url`. */
function normalizeEndpointUrl(url: string): string {
  return url.trim().replace(/\/+$/, "");
}

/** Mirrors asb-core's routing decision, including Responses field filtering. */
export type GatewayProfileInput = Pick<ProviderDraft,
  "app" | "routeMode" | "upstreamProtocol" | "baseUrl" | "responsesOptions" | "connection" | "authentication"> & {
  modelOptions?: ProviderDraft["modelOptions"];
  subagentRoute?: CodexSubagentRoute | null;
  modelRoutes?: readonly CodexModelRoute[];
};

export function requiresGateway(profile: GatewayProfileInput): boolean {
  if (profile.app === "claude" && profile.connection?.claudeNative) return false;
  const normalizedBase = profile.baseUrl === null ? null : normalizeEndpointUrl(profile.baseUrl);
  return profile.routeMode === "custom" && profile.upstreamProtocol !== null && (
    Boolean(profile.subagentRoute || profile.modelRoutes?.length)
    || (profile.modelOptions?.kind === "claude" && Object.values(profile.modelOptions.routes).some((route) => route?.mode === "provider"))
    || profile.upstreamProtocol !== NATIVE_PROTOCOL[profile.app]
    || (profile.upstreamProtocol === "responses" && profile.responsesOptions?.requestMode === "minimal")
    || (profile.app === "codex" && profile.authentication != null
      && profile.authentication !== "bearer")
    || (profile.app === "claude" && Boolean(profile.connection?.providerType
      || profile.connection?.authBinding?.source === "managed_account"))
    || Boolean(profile.connection?.claudeBilling)
    || Boolean(profile.connection?.claudePromptCacheKey)
    || Boolean(profile.connection?.isFullUrl)
    || Boolean(profile.connection?.customUserAgent?.trim())
    || Boolean(profile.connection?.localProxyRequestOverrides
      && (Object.keys(profile.connection.localProxyRequestOverrides.headers).length > 0
        || profile.connection.localProxyRequestOverrides.body !== null))
    || (profile.connection?.endpointAutoSelect !== false
      && Object.keys(profile.connection?.customEndpoints ?? {}).some((url) =>
        normalizeEndpointUrl(url) !== normalizedBase))
  );
}
