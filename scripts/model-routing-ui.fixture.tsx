import { useState } from "react";
import { createRoot } from "react-dom/client";
import type { AppKind, CodexCatalogEntry, CodexProviderRecord, ProviderRecord } from "../src/api/client";
import { ModelRoutingWorkspace } from "../src/components/model-routing/ModelRoutingWorkspace";
import type { RouteInventory, RouteRole } from "../src/components/model-routing/model";
import { applyLanguagePreference } from "../src/i18n";
import "../src/styles/tokens.css";
import "../src/styles/base.css";

const ids = {
  codexA: "11111111-1111-4111-8111-111111111111",
  codexB: "22222222-2222-4222-8222-222222222222",
  claudeA: "33333333-3333-4333-8333-333333333333",
  claudeB: "44444444-4444-4444-8444-444444444444",
};
const calls: { command: string; args: unknown }[] = [];
const entries: { app: AppKind; profileId: string; role: RouteRole }[] = [];
const providerEntries: AppKind[] = [];
const initialActiveIds = { codex: ids.codexA, claude: ids.claudeA };

function catalogEntry(id: string): CodexCatalogEntry {
  return { id, contextWindow: 32768, maxOutputTokens: 4096, functionTools: false, customTools: false,
    toolSearch: false, reasoning: false, defaultReasoningLevel: "none", supportedReasoningLevels: ["none"],
    images: false, compact: false };
}

function codex(id: string, name: string, position: number): CodexProviderRecord {
  const subagentRoute = id === ids.codexA ? { profileId: ids.codexB, model: "shared-model" } : null;
  return { profile: { id, name, routeMode: subagentRoute ? "gateway" : "direct",
    endpoint: "https://codex.fixture.invalid/v1", apiKey: "isolated-fixture-key", connection: {},
    upstream: "responses", requestMode: "standard", defaultModel: "shared-model",
    catalog: ["shared-model", "other-model"].map(catalogEntry), modelRoutes: [], subagentRoute,
    capabilities: { responses: true, compact: false, models: true, chatCompletions: false,
      alphaSearch: false, imageGeneration: false, imageEdit: false, functionTools: false,
      customTools: false, toolSearch: false, reasoning: false, chatReasoning: { kind: "unsupported" } } },
    position, fileHash: `fixture-${id}`, parameters: { settings: {} }, notes: null, websiteUrl: null, usageQuery: null };
}

function claude(id: string, name: string, position: number): ProviderRecord {
  const primary = id === ids.claudeA;
  return { profile: { id, app: "claude", name, routeMode: "custom", baseUrl: "https://claude.fixture.invalid/v1",
    apiKey: "isolated-fixture-key", connection: {}, upstreamProtocol: "anthropicMessages", responsesOptions: null,
    maxOutputTokens: null, model: "shared-model", parameters: { settings: {} }, websiteUrl: null,
    modelOptions: { kind: "claude", primaryOneM: primary,
      routes: { haiku: null,
        sonnet: primary ? { mode: "primary", model: "sonnet-local", oneM: false } : null,
        opus: primary ? { mode: "provider", profileId: ids.claudeB, model: "shared-model", oneM: true } : null,
        fable: null,
        subagent: primary ? { mode: "provider", profileId: id, model: "local-subagent", oneM: false } : null },
      availableModels: ["shared-model", "other-model", "sonnet-local", "local-subagent"] } },
    position, fileHash: `fixture-${id}` };
}

function initialInventory(scenario: string | null): RouteInventory {
  if (scenario === "empty") return { codex: [], profiles: [], activeIds: {} };
  const inventory: RouteInventory = { codex: [codex(ids.codexA, "Codex A", 100), codex(ids.codexB, "Codex B", 200)],
    profiles: [claude(ids.claudeA, "Claude A", 100), claude(ids.claudeB, "Claude B", 200)], activeIds: { ...initialActiveIds } };
  return scenario === "missing" ? { ...inventory,
    codex: inventory.codex.filter(({ profile }) => profile.id !== ids.codexB),
    profiles: inventory.profiles.filter(({ profile }) => profile.id !== ids.claudeB) } : inventory;
}

Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (command: string, args: unknown): Promise<never> => {
  calls.push({ command, args });
  throw new Error(`The read-only overview must not invoke ${command}`);
} } });
applyLanguagePreference("en-US");

function Fixture() {
  const query = new URLSearchParams(location.search);
  const [inventory, setInventory] = useState(() => initialInventory(query.get("scenario")));
  const [busy, setBusy] = useState(false);
  Object.assign(window, { routeTest: { ids, calls, entries, providerEntries, initialActiveIds,
    setLanguage: applyLanguagePreference, setBusy,
    removeProvider: (id: string) => setInventory((value) => ({ ...value,
      codex: value.codex.filter(({ profile }) => profile.id !== id),
      profiles: value.profiles.filter(({ profile }) => profile.id !== id) })),
    snapshot: () => structuredClone(inventory),
  } });
  return <main style={{ padding: 24 }}><ModelRoutingWorkspace inventory={inventory}
    initialApp={query.get("client") === "claude" ? "claude" : "codex"} busy={busy}
    runtimeSummary={<p data-testid="runtime-observation">Isolated runtime observation. No gateway listener is started.</p>}
    onEdit={(app, profileId, role) => entries.push({ app, profileId, role })}
    onProviders={(app) => providerEntries.push(app)} /></main>;
}

createRoot(document.getElementById("root")!).render(<Fixture />);
