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

export const providerCatalog = snapshot as { source: string; sourceSha256: string; generatedAt: string; providers: ProviderPreset[] };
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

const presetsByEndpoint = new Map<string, ProviderPreset[]>();
for (const preset of providerCatalog.providers) {
  const key = endpointKey(preset.endpoint);
  if (!key) continue;
  const presets = presetsByEndpoint.get(key);
  if (presets) presets.push(preset);
  else presetsByEndpoint.set(key, [preset]);
}

export function providerPreset(endpoint: string | null | undefined, protocol?: UpstreamProtocol): ProviderPreset | undefined {
  const key = endpointKey(endpoint ?? "");
  return key ? presetsByEndpoint.get(key)?.find((preset) => !protocol || preset.protocol === protocol) : undefined;
}

export function catalogModel(endpoint: string, protocol: UpstreamProtocol, id: string): CatalogModel | undefined {
  const key = endpointKey(endpoint);
  const presets = key ? presetsByEndpoint.get(key) : undefined;
  let match: CatalogModel | undefined;
  for (const preset of presets ?? []) {
    if (preset.protocol !== protocol) continue;
    const model = preset.models.find((item) => item.id === id);
    if (!model) continue;
    // Some subscription plans publish the same API root with different limits.
    if (match && JSON.stringify(match) !== JSON.stringify(model)) return undefined;
    match = model;
  }
  return match;
}
