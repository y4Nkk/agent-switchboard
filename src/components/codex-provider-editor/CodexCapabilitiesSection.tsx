import type { CodexChatReasoning } from "../../api/client";
import { useI18n } from "../../i18n";
import { Table } from "../Table";
import { ProviderValue } from "../provider-editor/ProviderValue";
import type { CodexEditorState } from "./useCodexProviderEditor";

const operations = [
  ["models", "codex.capability.models"], ["chatCompletions", "codex.capability.chatCompletions"],
  ["alphaSearch", "codex.capability.alphaSearch"], ["imageGeneration", "codex.capability.imageGeneration"],
  ["imageEdit", "codex.capability.imageEdit"],
] as const;

function ChatReasoning({ editor, busy }: { editor: CodexEditorState; busy: boolean }) {
  const { t } = useI18n();
  const value = editor.draft.capabilities.chatReasoning;
  const update = (next: CodexChatReasoning) => editor.setDraft((current) => ({ ...current,
    capabilities: { ...current.capabilities, chatReasoning: next },
  }));
  const patch = (key: string, next: string) => {
    if (value.kind === "configured") update({ ...value, [key]: key === "separateOutput" ? next === "true" : next });
  };
  const label = (option: string) => {
    if (option === "true" || option === "false") return t(option === "true" ? "providerSetup.enabled" : "providerSetup.disabled");
    return ({ none: "—", enableThinking: "enable_thinking", reasoningEffort: "reasoning_effort",
      reasoningObject: "reasoning.effort", deepSeek: "DeepSeek", openRouter: "OpenRouter" } as Record<string, string>)[option] ?? option;
  };
  return <div className="asb-provider-advanced-group">
    <div className="asb-field"><span>{t("codex.chatReasoning.title")}</span>
      <ProviderValue label={t("codex.chatReasoning.title")} value={value.kind} disabled={busy}
        display={t(value.kind === "configured" ? "providerSetup.configured" : "codex.chatReasoning.unused")}
        options={[{ value: "unsupported", label: t("codex.chatReasoning.unused") }, { value: "configured", label: t("codex.chatReasoning.configure") }]}
        onChange={(kind) => update(kind === "unsupported" ? { kind } : { kind: "configured", thinkingParameter: "none",
          separateOutput: false, effortParameter: "none", effortMode: "passthrough" })} />
    </div>
    {value.kind === "configured" && <Table ariaLabel={t("codex.chatReasoning.title")}
      rows={[
        { key: "thinkingParameter", label: t("codex.chatReasoning.thinkingParam"), value: value.thinkingParameter, options: ["none", "thinking", "enableThinking"] },
        { key: "separateOutput", label: t("codex.chatReasoning.separateOutput"), value: String(value.separateOutput), options: ["true", "false"] },
        { key: "effortParameter", label: t("codex.chatReasoning.effortParam"), value: value.effortParameter, options: ["none", "reasoningEffort", "reasoningObject"] },
        { key: "effortMode", label: t("codex.chatReasoning.effortMode"), value: value.effortMode, options: ["passthrough", "lowHigh", "deepSeek", "openRouter", "catalog"] },
      ]} rowKey={({ key }) => key} columns={[
        { key: "label", header: t("providerSetup.parameter"), render: (row) => row.label },
        { key: "value", header: t("providerSetup.value"), render: (row) => <ProviderValue label={row.label} value={row.value}
          display={label(row.value)} disabled={busy} options={row.options.map((option) => ({ value: option, label: label(option) }))}
          onChange={(next) => patch(row.key, next)} /> },
      ]} />}
    <p className="asb-field-help">{t("codex.chatReasoning.note")}</p>
  </div>;
}

export function CodexCapabilitiesSection({ editor, busy }: { editor: CodexEditorState; busy: boolean }) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  return <section className="asb-provider-advanced-group" aria-label={t("codex.capabilities.title")}>
    <h4 className="asb-provider-advanced-heading">{t("codex.capabilities.title")}</h4>
    <p className="asb-field-help">{t("providerSetup.capabilityHelp")}</p>
    <Table ariaLabel={t("codex.capabilities.operations")} rows={[...operations]} rowKey={([key]) => key} columns={[
      { key: "name", header: t("providerSetup.parameter"), render: ([, label]) => t(label) },
      { key: "value", header: t("providerSetup.value"), render: ([key, label]) => <ProviderValue label={t(label)} disabled={busy}
        value={String(draft.capabilities[key])} display={t(draft.capabilities[key] ? "providerSetup.enabled" : "providerSetup.disabled")}
        options={[{ value: "true", label: t("providerSetup.enabled") }, { value: "false", label: t("providerSetup.disabled") }]}
        onChange={(value) => setDraft((current) => ({ ...current, capabilities: { ...current.capabilities, [key]: value === "true" } }))} /> },
    ]} />
    {draft.upstream === "chatCompletions" && draft.capabilities.reasoning && <ChatReasoning editor={editor} busy={busy} />}
  </section>;
}
