import { invoke } from "./client";
import type { AppKind } from "./shared";
import type { LocalizedMessage } from "./switching";
import type { CodexCatalogEntry, CodexProviderDraft, ProviderDraft } from "./providers";
import type { UsageQuery } from "./usage";

export interface ProviderConversionRequest {
  sourceApp: AppKind;
  sourceProfileId: string;
  expectedSourceFileHash: string;
  targetApp: AppKind;
}

export interface ConversionCatalogEntry extends Omit<CodexCatalogEntry, "contextWindow" | "maxOutputTokens"> {
  contextWindow: number | null;
  maxOutputTokens: number | null;
}

/** An editing seed, never accepted by either persistent provider store. */
export interface CodexConversionDraft extends Omit<CodexProviderDraft, "catalog"> {
  catalog: ConversionCatalogEntry[];
}

export interface ProviderConversion {
  source: { app: AppKind; id: string; name: string; fileHash: string };
  targetApp: AppKind;
  status: "ready" | "needsInput" | "blocked";
  blockedReason: LocalizedMessage | null;
  notices: LocalizedMessage[];
  seed: { app: "claude"; draft: ProviderDraft } | { app: "codex"; draft: CodexConversionDraft } | null;
  /** Copied only on explicit selection. Refresh cadence is already zero. */
  usageQuery: UsageQuery | null;
}

export function prepareProviderConversion(request: ProviderConversionRequest): Promise<ProviderConversion> {
  return invoke("prepare_provider_conversion", { request });
}
