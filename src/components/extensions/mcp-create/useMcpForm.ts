import { uiMessage } from "../../../i18n/errors";
import { useRef, useState } from "react";
import type { AppKind, ExtensionDraft } from "../../../api/client";
import type { McpRegistrySource } from "../../../api/extensions/mcp";
import type { McpRegistryEntry } from "../../../api/extensions/registry";
import { materializeMcpDraft, validateMcpSource, type PutSecret } from "./mcp-draft";
import { emptyServer, formatMcpJson, jsonServiceName, parseMcpJson, renameJsonService, type CreateSource } from "./mcp-json";
import { validateUniqueName, uniquePresetName } from "./mcp-name";
import { buildMetadata, metadataDraft, type MetadataDraft } from "./metadata";
import { presetMetadata, presetServer } from "./presets";
import { useMcpSubmission } from "./useMcpSubmission";

export interface NewMcpFormProps {
  busy: boolean;
  onCancel: () => void;
  existingNames?: string[];
  registryEntry?: McpRegistryEntry & { source: McpRegistrySource };
  onBusyChange?: (busy: boolean) => void;
  onPutSecret: PutSecret;
  onSave: (draft: ExtensionDraft, clients: AppKind[]) => Promise<boolean>;
}

const DEFAULT_CLIENTS: AppKind[] = ["codex", "claude"];
interface EditorDocument { name: string; json: string }
const EMPTY_DOCUMENT: EditorDocument = { name: "", json: "" };

async function saveNewMcp(props: NewMcpFormProps, document: EditorDocument, clients: AppKind[], metadata: MetadataDraft) {
  const source = parseMcpJson(document.json, document.name);
  validateUniqueName(source.name, props.existingNames ?? []);
  validateMcpSource(source, clients);
  const registry = props.registryEntry?.source;
  const display = buildMetadata(metadata);
  const mcpMetadata = registry && source.server.type === "http" && source.server.url === registry.endpoint
    ? { ...display, registry } : display;
  const draft = await materializeMcpDraft(source, props.onPutSecret);
  if (!await props.onSave({ ...draft, ...(mcpMetadata ? { mcpMetadata } : {}) }, [...clients])) {
    throw uiMessage("mcp.error.saveIncomplete");
  }
}

export function useMcpForm(props: NewMcpFormProps) {
  const entry = props.registryEntry;
  const [document, setDocument] = useState(() => entry ? {
    name: uniquePresetName((entry.name.split("/").at(-1) ?? "mcp").replace(/[^A-Za-z0-9_-]/g, "-").slice(0, 56), props.existingNames ?? []),
    json: formatMcpJson({ type: "http", url: entry.source.endpoint, headers: {} }),
  } : EMPTY_DOCUMENT);
  const [clients, setClients] = useState(entry ? [] : [...DEFAULT_CLIENTS]);
  const [metadata, setMetadata] = useState(() => metadataDraft(entry ? {
    displayName: entry.title, description: entry.description, docs: entry.docs,
  } : undefined));
  const baseline = useRef(JSON.stringify({ document, clients, metadata }));
  const dirty = baseline.current !== JSON.stringify({ document, clients, metadata });
  const [preset, setPreset] = useState("custom");
  const [wizard, setWizard] = useState<CreateSource | null>(null);
  const [focusJson, setFocusJson] = useState(false);
  const submission = useMcpSubmission(props.busy, async () => {
    await saveNewMcp(props, document, clients, metadata);
    setDocument(EMPTY_DOCUMENT);
    setMetadata(metadataDraft());
    setClients([...DEFAULT_CLIENTS]);
    setPreset("custom");
    setFocusJson(true);
  }, props.onBusyChange);
  const { setError } = submission;
  const changeJson = (json: string) => {
    setDocument((current) => ({ json, name: jsonServiceName(json) ?? current.name }));
    setPreset("custom");
    setError(null);
  };
  const changeName = (name: string) => {
    setDocument((current) => ({ name, json: renameJsonService(current.json, name) }));
    setError(null);
  };
  const changeMetadata = (next: MetadataDraft) => { setMetadata(next); setError(null); };
  const selectPreset = (id: string) => {
    setDocument(id === "custom" ? EMPTY_DOCUMENT : {
      name: uniquePresetName(id, props.existingNames ?? []), json: formatMcpJson(presetServer(id)),
    });
    setMetadata(metadataDraft(id === "custom" ? undefined : presetMetadata(id)));
    setPreset(id);
    setError(null);
  };
  const openWizard = () => {
    try {
      setWizard(document.json.trim() ? parseMcpJson(document.json, document.name)
        : { name: document.name, server: emptyServer("stdio") });
      setError(null);
    } catch (caught) { setError(caught); }
  };
  const closeWizard = () => { setWizard(null); setFocusJson(true); };
  const applyWizard = (source: CreateSource) => {
    setDocument({ name: source.name, json: formatMcpJson(source.server) });
    setPreset("custom");
    setError(null);
    closeWizard();
  };
  const toggleClient = (client: AppKind) => {
    setClients((current) => DEFAULT_CLIENTS.filter((entry) => entry === client ? !current.includes(entry) : current.includes(entry)));
    setError(null);
  };
  return { ...document, ...submission, clients, metadata, preset, wizard, focusJson, dirty,
    changeJson, changeName, changeMetadata, selectPreset, openWizard, closeWizard, applyWizard, toggleClient };
}
