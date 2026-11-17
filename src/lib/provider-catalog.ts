import snapshot from "./provider-catalog.generated.json" with { type: "json" };
import type { AppKind, CodexReasoningLevel, UpstreamProtocol } from "../api/client";

export interface CatalogModel {
  id: string;
  name: string;
  contextWindow: number | null;
  maxOutputTokens: number | null;
  reasoning: boolean;
  reasoningLevels: CodexReasoningLevel[];
  functionTools: boolean;
  images: boolean;
}

export interface ProviderPreset {
  id: string;
  name: string;
  endpoint: string;
  protocol: UpstreamProtocol;
  website: string | null;
  models: CatalogModel[];
}

export const providerCatalog = snapshot as { source: string; generatedAt: string; providers: ProviderPreset[] };
export const providerPresets = (app: AppKind) => providerCatalog.providers
  .filter((preset) => app === "claude" || preset.protocol !== "geminiGenerateContent");

/** Matching includes the entire API root: different plans under one host never share limits. */
function endpointKey(raw: string): string | null {
  try {
    const url = new URL(raw);
    if (url.protocol !== "https:" && url.protocol !== "http:") return null;
    if (url.search || url.hash || url.username || url.password) return null;
    return `${url.protocol}//${url.host.toLowerCase()}${url.pathname.replace(/\/+$/, "")}`;
  } catch { return null; }
}

export function providerPreset(endpoint: string | null | undefined): ProviderPreset | undefined {
  const key = endpointKey(endpoint ?? "");
  return key ? providerCatalog.providers.find((preset) => endpointKey(preset.endpoint) === key) : undefined;
}

export function catalogModel(endpoint: string, protocol: UpstreamProtocol, id: string): CatalogModel | undefined {
  const preset = providerPreset(endpoint);
  return preset?.protocol === protocol ? preset.models.find((model) => model.id === id) : undefined;
}
