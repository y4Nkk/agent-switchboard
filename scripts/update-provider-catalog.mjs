import { readFile, writeFile } from "node:fs/promises";

const source = "https://models.dev/api.json";
const presets = [
  ["openai", "responses", "https://api.openai.com/v1"],
  ["anthropic", "anthropicMessages", "https://api.anthropic.com/v1"],
  ["deepseek", "chatCompletions"],
  ["moonshotai", "chatCompletions"],
  ["moonshotai-cn", "chatCompletions"],
  ["zhipuai", "chatCompletions"],
  ["zai", "chatCompletions"],
  ["siliconflow", "chatCompletions"],
  ["siliconflow-cn", "chatCompletions"],
  ["openrouter", "chatCompletions"],
  ["google", "geminiGenerateContent", "https://generativelanguage.googleapis.com/v1beta"],
];
const levels = new Set(["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"]);
const response = process.argv[2] ? null : await fetch(source, { headers: { "User-Agent": "Mozilla/5.0" } });
if (response && !response.ok) throw new Error(`Model catalog: HTTP ${response.status}`);
const data = response ? await response.json() : JSON.parse(await readFile(process.argv[2], "utf8"));
const positive = (value) => Number.isSafeInteger(value) && value > 0 ? value : null;
const providers = presets.map(([id, protocol, endpoint]) => {
  const provider = data[id];
  if (typeof provider?.name !== "string" || !provider.name || !provider.models
    || typeof provider.models !== "object" || Array.isArray(provider.models)
    || !(endpoint ?? provider.api)) throw new Error(`Missing provider ${id}`);
  const models = Object.values(provider.models)
    .filter((model) => model.modalities?.output?.includes("text") && model.status !== "deprecated")
    .map((model) => ({
      id: model.id, name: model.name,
      contextWindow: positive(model.limit?.context), maxOutputTokens: positive(model.limit?.output),
      reasoning: model.reasoning === true,
      reasoningLevels: [...new Set((model.reasoning_options ?? []).filter((option) => option.type === "effort")
        .flatMap((option) => option.values ?? []).filter((level) => levels.has(level)))],
      functionTools: model.tool_call === true, images: model.modalities?.input?.includes("image") === true,
    }));
  if (!models.length || models.some((model) => typeof model.id !== "string" || !model.id
    || typeof model.name !== "string" || !model.name)) throw new Error(`Invalid models in ${id}`);
  return { id, name: provider.name, endpoint: endpoint ?? provider.api, protocol, website: provider.doc ?? null, models };
});
const snapshot = { source, generatedAt: new Date().toISOString().slice(0, 10), providers };
await writeFile(new URL("../src/lib/provider-catalog.generated.json", import.meta.url), `${JSON.stringify(snapshot)}\n`);
console.log(`Provider catalog: ${providers.length} providers, ${providers.reduce((count, provider) => count + provider.models.length, 0)} models`);
