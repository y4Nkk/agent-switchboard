import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";

const preferred = ["openai", "anthropic", "deepseek", "moonshotai", "moonshotai-cn", "zhipuai", "zai",
  "siliconflow", "siliconflow-cn", "openrouter", "google"];
const knownRoots = {
  openai: "https://api.openai.com/v1",
  anthropic: "https://api.anthropic.com",
  google: "https://generativelanguage.googleapis.com/v1beta",
};
const protocols = new Map([
  ["@ai-sdk/openai", "responses"], ["@ai-sdk/anthropic", "anthropicMessages"],
  ["@ai-sdk/openai-compatible", "chatCompletions"], ["@openrouter/ai-sdk-provider", "chatCompletions"],
  ["@ai-sdk/google", "geminiGenerateContent"],
]);
const levels = new Set(["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"]);
const positive = (value) => Number.isSafeInteger(value) && value > 0 ? value : null;

function catalogModel(model) {
  return {
    id: model.id, name: model.name,
    contextWindow: positive(model.limit?.context), maxOutputTokens: positive(model.limit?.output),
    reasoning: model.reasoning === true,
    reasoningLevels: [...new Set((model.reasoning_options ?? []).filter((option) => option.type === "effort")
      .flatMap((option) => option.values ?? []).filter((level) => levels.has(level)))],
    functionTools: model.tool_call === true, images: model.modalities?.input?.includes("image") === true,
  };
}

function catalogProvider(id, provider) {
  if (!provider || typeof provider !== "object") throw new Error(`Invalid provider ${id}`);
  const endpoint = knownRoots[id] ?? provider.api;
  const protocol = protocols.get(provider.npm);
  if (typeof endpoint !== "string" || !protocol || endpoint.includes("${") || !URL.canParse(endpoint)) return null;
  const url = new URL(endpoint);
  if (url.protocol !== "https:" || url.search || url.hash || url.username || url.password) return null;
  if (!provider.models || typeof provider.models !== "object" || Array.isArray(provider.models)) {
    throw new Error(`Invalid model catalog for ${id}`);
  }
  const models = Object.values(provider.models)
    .filter((model) => model.modalities?.output?.includes("text") && model.status !== "deprecated")
    .map(catalogModel);
  if (!models.length) return null;
  if (!provider.name || models.some((model) => typeof model.id !== "string" || !model.id
    || typeof model.name !== "string" || !model.name)) throw new Error(`Invalid models in ${id}`);
  const base = protocol === "anthropicMessages" ? endpoint.replace(/\/v1\/?$/, "") : endpoint;
  return { id, name: provider.name, endpoint: base, protocol, website: provider.doc ?? null, models };
}

const [inputPath, source] = process.argv.slice(2);
if (!inputPath || !source?.trim()) {
  throw new Error("Usage: node scripts/update-provider-catalog.mjs <local-models.dev.json> <source>");
}
const raw = await readFile(inputPath);
const data = JSON.parse(raw.toString("utf8"));
if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("Invalid models.dev snapshot");
const ids = [...new Set([...preferred, ...Object.keys(data).sort()])];
const providers = ids.map((id) => catalogProvider(id, data[id])).filter(Boolean);
if (!preferred.every((id) => providers.some((provider) => provider.id === id))) throw new Error("Missing preferred provider");
const sourceSha256 = createHash("sha256").update(raw).digest("hex");
const snapshot = { source: source.trim(), sourceSha256, generatedAt: new Date().toISOString().slice(0, 10), providers };
await writeFile(new URL("../src/lib/provider-catalog.generated.json", import.meta.url), `${JSON.stringify(snapshot)}\n`);
console.log(`Provider catalog: ${providers.length} providers, ${providers.reduce((count, provider) => count + provider.models.length, 0)} models`);
