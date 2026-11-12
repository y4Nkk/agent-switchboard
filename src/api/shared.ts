/* Client-boundary primitives shared by every api domain module. */

export type AppKind = "codex" | "claude";
export type RouteMode = "official" | "custom";
/** The provider endpoint's actual wire protocol. */
export type UpstreamProtocol = "responses" | "chatCompletions" | "anthropicMessages" | "geminiGenerateContent";
export type AuthenticationScheme = "bearer" | "xApiKey" | "xGoogApiKey";

/** Class of one outbound transport failure; the renderer owns the text. */
export type CodexTransportClass = "dns" | "timeout" | "connect" | "tls" | "other";

export interface ResponsesOptions {
  requestMode: "standard" | "minimal";
}

export interface CodexModelSettings {
  contextWindow: number | null;
}

export type ClaudeRouteRole = "haiku" | "sonnet" | "opus" | "fable" | "subagent";
export type ClaudeModelRoute =
  | { mode: "primary"; model: string; oneM: boolean }
  | { mode: "provider"; profileId: string; model: string; oneM: boolean };
export interface ClaudeModelSettings {
  primaryOneM: boolean;
  routes: Record<ClaudeRouteRole, ClaudeModelRoute | null>;
  availableModels: string[] | null;
  displayNames?: { haiku: string | null; sonnet: string | null; opus: string | null; fable: string | null } | null;
}

export type ModelOptions =
  | ({ kind: "codex" } & CodexModelSettings)
  | ({ kind: "claude" } & ClaudeModelSettings);

export interface CommandError {
  code: string;
  message: string;
  /** Catalog key for the app-owned explanation of the current language;
   * present for app-owned explanations. External diagnostics retain `message`, which
   * remains the scrubbed raw diagnostic and the fallback rendering. */
  messageKey?: string;
  /** Interpolation values for `messageKey` placeholders. */
  params?: Record<string, string | number>;
}
