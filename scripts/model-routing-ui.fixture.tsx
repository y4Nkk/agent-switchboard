import { useState } from "react";
import { createRoot } from "react-dom/client";
import { ModelRoutingWorkspace } from "../src/components/model-routing/ModelRoutingWorkspace";
import { applyLanguagePreference } from "../src/i18n";
import "../src/styles/tokens.css";
import "../src/styles/base.css";

const roles = { haiku: null, sonnet: null, opus: null, fable: null, subagent: null };
const codex = (id: string) => ({ profile: { id, name: id, routeMode: "gateway", endpoint: `https://${id}.invalid`,
  apiKey: "isolated-key", connection: {}, upstream: "responses", requestMode: "standard", defaultModel: "shared-model",
  catalog: ["shared-model", "other-model"].map((id) => ({ id, supportedReasoningLevels: [] })),
  modelRoutes: [], subagentRoute: null, capabilities: {} }, fileHash: id, parameters: { settings: {} } });
const claude = (id: string) => ({ profile: { id, app: "claude", name: id, routeMode: "custom", baseUrl: `https://${id}.invalid`,
  apiKey: "isolated-key", connection: {}, model: "shared-model", parameters: { settings: {} },
  modelOptions: { kind: "claude", primaryOneM: false, routes: { ...roles, ...(id === "Claude A" ? { sonnet: { mode: "primary", model: "unique-only-tier", oneM: false } } : {}) }, availableModels: ["shared-model", "other-model"] } }, fileHash: id });
const records = { codex: [codex("Codex A"), codex("Codex B")], profiles: [claude("Claude A"), claude("Claude B")] };
const calls: { command: string; args: any }[] = [];
let kind = "saveOnly";
let serial = 0;
let cancelFailures = 0;
let delayNextPrepare = false;
let resolvePreparation: (() => void) | null = null;
const prepared = new Map<string, any>();
const preview = { content: "isolated preview", contentHash: "before", renderedHash: "after", preview: {
  target: "isolated/client-config", backupDir: "isolated/backup", changes: [
    { key: "env.ANTHROPIC_BASE_URL", kind: "set", before: "https://A.invalid", after: "http://127.0.0.1:47821" },
  ], warnings: [{ key: "warnings.gateway.freeform", params: { detail: "Isolated warning remains visible." }, text: "Isolated warning remains visible." }], preservedKeys: [] } };
(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
  calls.push({ command, args });
  if (command.startsWith("prepare_")) {
    const id = `isolated-${++serial}`;
    prepared.set(id, args);
    if (delayNextPrepare) {
      delayNextPrepare = false;
      await new Promise<void>((resolve) => { resolvePreparation = resolve; });
      resolvePreparation = null;
    }
    return { preparationId: id, kind, preview: kind === "saveAndApply" ? preview : null };
  }
  if (command === "cancel_profile_save") {
    if (cancelFailures > 0) { cancelFailures--; throw { code: "isolated-cancel-failure", message: "Isolated cancellation failure" }; }
    prepared.delete(args.preparationId); return;
  }
  if (command.startsWith("commit_")) {
    const { profileId, draft } = prepared.get(args.preparationId);
    prepared.delete(args.preparationId);
    const rows = command === "commit_codex_profile_save" ? records.codex : records.profiles;
    const index = rows.findIndex(({ profile }) => profile.id === profileId);
    const saved = { ...rows[index], profile: { ...rows[index].profile, ...draft, id: profileId }, fileHash: `saved-${serial}` };
    (rows as any)[index] = saved;
    return saved;
  }
  throw new Error(`Unexpected mocked command: ${command}`);
} };
applyLanguagePreference("en-US");
function Fixture() {
  const [busy, setBusy] = useState(false);
  const [inventory, setInventory] = useState({ ...records, activeIds: { codex: "Codex A", claude: "Claude A" } });
  (window as any).routeTest = { calls,
    failCancels: (count: number) => { cancelFailures = count; },
    delayNext: () => { delayNextPrepare = true; },
    resolveDelayed: () => resolvePreparation?.(),
    setLanguage: applyLanguagePreference, setKind: (next: string) => { kind = next; },
    switchActive: (client: string, id: string) => setInventory((value) => ({ ...value, activeIds: { ...value.activeIds, [client]: id } })),
    removeClaudeTarget: () => setInventory((value) => ({ ...value, profiles: value.profiles.filter(({ profile }) => profile.id !== "Claude B") })),
    snapshot: () => ({ ...inventory, pendingCount: prepared.size, waitingPreparation: !!resolvePreparation }),
  };
  return <main style={{ padding: 24 }}><ModelRoutingWorkspace inventory={inventory as any} active initialApp={new URLSearchParams(location.search).get("client") === "claude" ? "claude" : "codex"}
    request={null} busy={busy} onBusy={setBusy} runtimeSummary={<p>Observed runtime stays independent of the editor.</p>}
    onProviders={() => {}} onApplied={async () => { setInventory((value) => ({ ...value, ...records })); }} /></main>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
