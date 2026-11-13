import { useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { useProviders } from "../src/app/useProviders";
import { ProviderEditor } from "../src/components/ProviderEditor";
import { CodexProviderEditor } from "../src/components/codex-provider-editor/CodexProviderEditor";
import { ProviderList } from "../src/components/ProviderList";
import { CodexProviderRow } from "../src/components/CodexProviderRows";
import { SortableProviderRows } from "../src/components/ProviderWorkspaceShell";
import { ProviderConversionBlocked } from "../src/components/provider-editor/ProviderConversionSummary";
import { applyLanguagePreference } from "../src/i18n";
import type { AppKind, CodexProviderRecord, ProviderConversion, ProviderRecord } from "../src/api/client";
import "../src/styles/tokens.css";
import "../src/styles/base.css";

const roles = { haiku: null, sonnet: null, opus: null, fable: null, subagent: null };
const capabilities = { responses: true, compact: false, models: false, chatCompletions: false,
  alphaSearch: false, imageGeneration: false, imageEdit: false, functionTools: false,
  customTools: false, toolSearch: false, reasoning: false, chatReasoning: { kind: "unsupported" as const } };
const catalogEntry = (id: string) => ({ id, contextWindow: 32768, maxOutputTokens: 2048,
  functionTools: false, customTools: false, toolSearch: false, reasoning: false,
  defaultReasoningLevel: "none" as const, supportedReasoningLevels: ["none" as const], images: false, compact: false });
const codex: CodexProviderRecord = { profile: { id: "codex-source", name: "Provider Alpha",
  routeMode: "gateway", endpoint: "https://alpha.invalid/v1", apiKey: "fixture-alpha-key", authentication: "bearer",
  connection: {}, upstream: "responses", requestMode: "standard", defaultModel: "fast",
  catalog: [catalogEntry("fast"), catalogEntry("other")], modelRoutes: [{ clientModel: "fast", upstreamModel: "vendor-fast" }],
  subagentRoute: null, capabilities }, parameters: { settings: {} }, notes: "Alpha notes", websiteUrl: null,
  usageQuery: null, position: 100, fileHash: "alpha-revision" };
const claude: ProviderRecord = { profile: { id: "claude-source", app: "claude", name: "Provider Beta",
  routeMode: "custom", baseUrl: "https://beta.invalid", apiKey: "fixture-beta-key", authentication: "xApiKey",
  connection: {}, upstreamProtocol: "anthropicMessages", model: "vendor-beta", responsesOptions: null,
  maxOutputTokens: null, modelOptions: { kind: "claude", primaryOneM: false, routes: roles, availableModels: null },
  parameters: { settings: {} }, websiteUrl: null, usageQuery: null }, position: 100, fileHash: "beta-revision" };
const official: ProviderRecord = { profile: { ...claude.profile, id: "official-source", name: "Official login",
  routeMode: "official", baseUrl: null, apiKey: "", upstreamProtocol: null, authentication: null, model: null,
  modelOptions: null }, position: 200, fileHash: "official-revision" };
const initialSources = JSON.stringify({ codex, claude, official });
const { id: _sourceIdentity, ...claudeDraft } = claude.profile;
const calls: { command: string; args: any }[] = [];
const created: any[] = [];
const pending = new Map<string, any>();
let failSave = false;
let delayPrepare = false;
let releasePrepare: (() => void) | null = null;
let delayedSaveCommand: string | null = null;
let releaseSave: (() => void) | null = null;

function conversionFor(sourceId: string): ProviderConversion {
  const fromCodex = sourceId === codex.profile.id;
  const record = fromCodex ? codex : sourceId === official.profile.id ? official : claude;
  const source = { app: fromCodex ? "codex" as const : "claude" as const, id: record.profile.id,
    name: record.profile.name, fileHash: record.fileHash };
  const targetApp = fromCodex ? "claude" as const : "codex" as const;
  const base = { source, targetApp, notices: [], usageQuery: { kind: "declarative" as const,
    url: "{{baseUrl}}/balance", remainingPath: "/remaining", refreshIntervalMinutes: 0 } };
  if (sourceId === official.profile.id) return { ...base, status: "blocked", seed: null,
    blockedReason: { key: "providers.conversion.blocked.official", params: {}, text: "Official login cannot be converted." } };
  if (fromCodex) return { ...base, status: "ready", blockedReason: null, seed: { app: "claude", draft: {
    ...claudeDraft, app: "claude", name: codex.profile.name, baseUrl: codex.profile.endpoint,
    apiKey: codex.profile.apiKey, authentication: "bearer", upstreamProtocol: "responses",
    responsesOptions: { requestMode: "standard" }, model: "vendor-fast",
    modelOptions: { kind: "claude", primaryOneM: false, routes: roles, availableModels: ["vendor-fast", "other"] },
    notes: codex.notes, usageQuery: null } } };
  return { ...base, status: "needsInput", blockedReason: null, seed: { app: "codex", draft: {
    name: claude.profile.name, endpoint: claude.profile.baseUrl!, apiKey: claude.profile.apiKey,
    authentication: "xApiKey", connection: {}, upstream: "anthropicMessages", requestMode: "standard",
    defaultModel: "vendor-beta", catalog: [{ ...catalogEntry("vendor-beta"), contextWindow: null, maxOutputTokens: null }],
    modelRoutes: [], subagentRoute: null, capabilities, parameters: { settings: {} },
    notes: null, websiteUrl: null, usageQuery: null } } };
}

function saveRecord(app: AppKind, id: string, draft: any) {
  if (failSave) { failSave = false; throw { code: "isolated-save-failed", message: "Isolated save failure" }; }
  if (!id || [codex.profile.id, claude.profile.id, official.profile.id].includes(id)) throw new Error("A create reused its source identity");
  const record = { profile: { ...draft, app, id }, fileHash: `${id}-revision`, position: 300,
    parameters: draft.parameters, notes: draft.notes, websiteUrl: draft.websiteUrl, usageQuery: draft.usageQuery };
  created.push(record);
  return record;
}

(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
  calls.push({ command, args: structuredClone(args) });
  if (command === delayedSaveCommand) {
    delayedSaveCommand = null;
    await new Promise<void>((resolve) => { releaseSave = resolve; });
    releaseSave = null;
  }
  if (command === "prepare_provider_conversion") {
    if (delayPrepare) {
      delayPrepare = false;
      await new Promise<void>((resolve) => { releasePrepare = resolve; });
      releasePrepare = null;
    }
    return conversionFor(args.request.sourceProfileId);
  }
  if (command === "get_provider_parameters_catalog") return { app: args.target,
    defaults: { settings: {} }, groups: [], specs: [], claudeModelPreferences: null };
  if (command === "resolve_provider_endpoints") return { requestUrl: `${args.request.baseUrl}/request`, modelsUrl: null, modelsError: null };
  if (command === "gateway_status") return { baseUrl: "http://127.0.0.1:47821" };
  if (command === "create_codex_profile") {
    if ("id" in args.draft) throw new Error("A new Codex draft leaked a stored identity");
    return saveRecord("codex", args.profileId, args.draft);
  }
  if (command === "prepare_profile_save") {
    if (args.target.kind !== "create" || "expectedFileHash" in args.target) throw new Error("Conversion attempted an existing-profile update");
    if ("id" in args.draft) throw new Error("A new Claude draft leaked a stored identity");
    const preparationId = `prepared-${pending.size}`;
    pending.set(preparationId, { id: args.target.profileId, draft: args.draft });
    return { preparationId, kind: "create", preview: null };
  }
  if (command === "commit_profile_save") {
    if (args.confirmWrite !== false) throw new Error("Conversion attempted live configuration write");
    const prepared = pending.get(args.preparationId);
    return saveRecord("claude", prepared.id, prepared.draft);
  }
  if (command === "cancel_profile_save") { pending.delete(args.preparationId); return; }
  throw new Error(`Unexpected fixture command: ${command}`);
} };
applyLanguagePreference("en-US");

function ConversionEditor({ providers, busy }: { providers: ReturnType<typeof useProviders>; busy: boolean }) {
  const session = providers.editorSession;
  if (!session) return null;
  const navigation = { profileId: session.profileId, section: session.section, routeRole: session.role,
    request: providers.editorRequest, inventory: { codex: [codex], profiles: [claude, official], activeIds: {} },
    onRequestHandled: providers.handleEditorRequest, onReplaceSession: providers.replaceEditorSession,
    onReload: providers.reloadEditor };
  if (session.app === "codex") return <CodexProviderEditor key={session.profileId} {...navigation} active source={session.source} busy={busy}
    userConfigModel={null} userConfigWarnings={[]} onSave={providers.saveCodexProfile}
    onSaveOfficial={providers.saveGenericProfile} onSwitchAccessMode={providers.switchCodexAccessMode}
    onSwitchClient={providers.newEditorFor} onCancel={providers.closeEditor} />;
  return <ProviderEditor key={session.profileId} {...navigation} active profile={null} initialApp="claude"
    conversion={session.conversion} busy={busy} officialTakenApps={[]} onOpenOfficial={() => {}}
    onSwitchClient={providers.newEditorFor} userConfigModel={null} userConfigWarnings={[]}
    onSave={providers.saveGenericProfile} onCancel={providers.closeEditor} />;
}

function SourceRows({ providers, busy }: { providers: ReturnType<typeof useProviders>; busy: boolean }) {
  const noop = () => {};
  return <section aria-label="Saved sources" className="asb-provider-list">
    <SortableProviderRows ids={[codex.profile.id]}>
      <CodexProviderRow record={codex} active={false} busy={busy} confirmationOpen={false}
        usageOpen={false} onToggleUsage={noop} onConfigureUsage={noop} onActivate={noop}
        onEdit={noop} onDelete={noop} onRoutes={noop} onConvert={() => providers.convertCodexProfile(codex)} />
    </SortableProviderRows>
    <ProviderList profiles={[claude.profile, official.profile]} activeProfileId={null}
      userConfigModel={null} userConfigWarnings={[]} busy={busy} activationCandidate={null}
      onConfirmSwitch={noop} onCancelActivation={noop} onEdit={noop} onDelete={noop} onRoutes={noop}
      onConfigureUsage={noop} onActivate={noop} onConvert={providers.convertProfile} />
  </section>;
}

function Fixture() {
  const [busy, setBusy] = useState(false);
  const [active, setActive] = useState(true);
  const workspaceRevision = useRef(0);
  const [app, setApp] = useState<AppKind>("codex");
  const [error, setError] = useState("");
  const [target, setTarget] = useState<string | null>(null);
  const providers = useProviders({ active, busy, appFilter: app, setAppFilter: setApp,
    records: [claude, official], codexOfficialRecords: [], targetProfileId: target, setTargetProfileId: setTarget,
    codexRecords: [codex], activeIds: {}, openEditorWorkspace: () => {}, returnToGateway: () => {},
    workspaceVersion: () => workspaceRevision.current,
    onError: (failure) => setError(failure.message || failure.code), clearError: () => setError(""), setBusy,
    invalidateCandidates: () => {}, clearCandidates: () => {}, refresh: async () => {}, setTargetProfile: setTarget });
  (window as any).conversionTest = { calls, created, setLanguage: applyLanguagePreference,
    delayNext: () => { delayPrepare = true; }, release: () => releasePrepare?.(),
    newClaude: () => providers.newEditorFor("claude"),
    setActive: (value: boolean) => { workspaceRevision.current += 1; setActive(value); },
    delaySave: (command: string) => { delayedSaveCommand = command; }, releaseSave: () => releaseSave?.(),
    failNextSave: () => { failSave = true; },
    snapshot: () => ({ busy, app, session: providers.editorSession, created, target,
      sourcesUnchanged: initialSources === JSON.stringify({ codex, claude, official }), waiting: releasePrepare !== null,
      waitingSave: releaseSave !== null }) };
  return <main style={{ height: "100vh", padding: 16, background: "var(--asb-content)", color: "var(--asb-text)" }}>
    {error && <p role="alert">{error}</p>}
    {!providers.editorSession && <SourceRows providers={providers} busy={busy} />}
    <ConversionEditor providers={providers} busy={busy} />
    {providers.blockedConversion && <ProviderConversionBlocked conversion={providers.blockedConversion} onClose={providers.dismissConversion} />}
  </main>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
