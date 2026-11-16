import type { ProviderDiagnostic } from "../../src/api/provider-request.ts";

/** Matched against a real Rust-serialized diagnostic, including nullable fields. */
export const providerDiagnostic = {
  kind: "rateLimit",
  endpoint: "https://example.invalid/v1/responses",
  transport: "http",
  status: 429,
  requestId: "fixture-request",
  retryAfter: "120",
  body: null,
  bodyTruncated: false,
  message: "Rate limited",
} satisfies ProviderDiagnostic;
