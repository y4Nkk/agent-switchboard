import { invoke } from "./client";
import type { ProviderConnectionOptions, ProviderModel } from "./providers";
import type { ResponsesOptions, UpstreamProtocol } from "./shared";

/** The connection under test, independent of saved provider metadata. */
export interface ProviderRequestConnection {
  app: import("./shared").AppKind;
  baseUrl: string;
  apiKey: string;
  connection: ProviderConnectionOptions;
  authentication?: import("./shared").AuthenticationScheme | null;
  upstreamProtocol: UpstreamProtocol;
  responsesOptions: ResponsesOptions | null;
  defaultModel: string | null;
}

export type ProviderRequestTarget =
  | { kind: "saved"; profileId: string }
  | { kind: "draft"; connection: ProviderRequestConnection };

/** Credential-free metadata for one backend-owned, single-use request. */
export interface ProviderRequestPreparation {
  requestId: string;
  endpoint: string;
  upstreamProtocol: UpstreamProtocol;
  defaultModel: string | null;
  prompt: string;
}

export type ProviderRequestOutcome =
  | "success"
  | "authenticationFailed"
  | "rateLimited"
  | "httpError"
  | "networkError"
  | "timeout"
  | "invalidResponse"
  | "cancelled";

export type ProviderDiagnosticKind =
  | "dns"
  | "tls"
  | "network"
  | "timeout"
  | "websocketUnsupported"
  | "endpoint"
  | "authentication"
  | "requestParameters"
  | "modelNotFound"
  | "rateLimit"
  | "upstream"
  | "responseParse"
  | "streamParse";

export interface ProviderDiagnostic {
  kind: ProviderDiagnosticKind;
  endpoint: string;
  transport: "http" | "websocket";
  status: number | null;
  requestId: string | null;
  retryAfter: string | null;
  body: string | null;
  bodyTruncated: boolean;
  message: string;
}

export interface ProviderRequestResult {
  outcome: ProviderRequestOutcome;
  status: number | null;
  latencyMs: number;
  /** The model reported by the provider, never a copy of the requested ID. */
  model: string | null;
  reply: string | null;
  error: string | null;
  diagnostic: ProviderDiagnostic | null;
  at: string;
}

/** The only provider-request command that accepts a connection. A saved
 * profile sends its id; an unsaved draft sends its connection once, and every
 * later command refers to it by the opaque token returned here. */
export function prepareProviderRequest(target: ProviderRequestTarget): Promise<ProviderRequestPreparation> {
  return invoke("prepare_provider_request", { target });
}

/** Models offered by the connection a preparation token stands for. The
 * backend resolves the credential from its prepared target, so no credential
 * travels with the call. */
export function fetchProviderRequestModels(requestId: string): Promise<ProviderModel[]> {
  return invoke<ProviderModel[]>("fetch_provider_request_models", { requestId });
}

export function executeProviderRequest(requestId: string, model: string): Promise<ProviderRequestResult> {
  return invoke("execute_provider_request", { request: { requestId, model } });
}

export function cancelProviderRequest(requestId: string): Promise<boolean> {
  return invoke("cancel_provider_request", { requestId });
}
