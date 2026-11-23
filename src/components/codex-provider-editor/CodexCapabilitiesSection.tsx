import type { CodexAutomaticOperation, CodexChatReasoning } from "../../api/client";
import { useI18n, type MessageKey, type TFunction } from "../../i18n";
import { Button } from "../Button";
import { Select } from "../Select";
import { Switch } from "../Switch";
import type { CodexEditorState } from "./useCodexProviderEditor";
import { AUTOMATIC_OPERATIONS, restoreCodexAutomaticOperation, setCodexOperation } from "./operations";

const operationLabels: Record<CodexAutomaticOperation, MessageKey> = {
  models: "codex.capability.models",
  chatCompletions: "codex.capability.chatCompletions",
  alphaSearch: "codex.capability.alphaSearch",
  imageGeneration: "codex.capability.imageGeneration",
  imageEdit: "codex.capability.imageEdit",
};

function optionLabel(option: string, t: TFunction): string {
  if (option === "true" || option === "false") return t(option === "true" ? "providerSetup.enabled" : "providerSetup.disabled");
  return ({ none: "—", enableThinking: "enable_thinking", reasoningEffort: "reasoning_effort",
    reasoningObject: "reasoning.effort", deepSeek: "DeepSeek", openRouter: "OpenRouter" } as Record<string, string>)[option] ?? option;
}

function ChatReasoning({ editor, busy }: { editor: CodexEditorState; busy: boolean }) {
  const { t } = useI18n();
  const value = editor.draft.capabilities.chatReasoning;
  const configured = value.kind === "configured";
  const update = (next: CodexChatReasoning) => editor.setDraft((current) => ({ ...current,
    capabilities: { ...current.capabilities, chatReasoning: next },
  }));
  const patch = (key: string, next: string) => {
    if (value.kind === "configured") update({ ...value, [key]: key === "separateOutput" ? next === "true" : next });
  };
  const select = (key: "thinkingParameter" | "effortParameter" | "effortMode", labelKey: MessageKey, options: readonly string[]) => <label className="asb-field"><span>{t(labelKey)}</span>
    <Select ariaLabel={t(labelKey)} disabled={busy} value={String(value.kind === "configured" ? value[key] : "")}
      options={options.map((option) => ({ value: option, label: optionLabel(option, t) }))}
      onChange={(next) => patch(key, next)} /></label>;
  return <div className="asb-provider-advanced-group">
    <div className="asb-provider-switch-row"><span>{t("codex.chatReasoning.title")}</span>
      <Switch checked={configured} disabled={busy} label={t("codex.chatReasoning.title")}
        onChange={(on) => update(on ? { kind: "configured", thinkingParameter: "none",
          separateOutput: false, effortParameter: "none", effortMode: "passthrough" } : { kind: "unsupported" })} />
    </div>
    {configured && <div className="asb-provider-field-grid">
      {select("thinkingParameter", "codex.chatReasoning.thinkingParam", ["none", "thinking", "enableThinking"])}
      <div className="asb-provider-switch-row"><span>{t("codex.chatReasoning.separateOutput")}</span>
        <Switch checked={value.separateOutput} disabled={busy} label={t("codex.chatReasoning.separateOutput")}
          onChange={(on) => patch("separateOutput", on ? "true" : "false")} /></div>
      {select("effortParameter", "codex.chatReasoning.effortParam", ["none", "reasoningEffort", "reasoningObject"])}
      {select("effortMode", "codex.chatReasoning.effortMode", ["passthrough", "lowHigh", "deepSeek", "openRouter", "catalog"])}
    </div>}
    <p className="asb-field-help">{t("codex.chatReasoning.note")}</p>
  </div>;
}

export function CodexCapabilitiesSection({ editor, busy }: { editor: CodexEditorState; busy: boolean }) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  return <section className="asb-provider-advanced-group" aria-label={t("codex.capabilities.title")}>
    <h4 className="asb-provider-advanced-heading">{t("codex.capabilities.title")}</h4>
    <p className="asb-field-help">{t("providerSetup.capabilityHelp")}</p>
    <div className="asb-provider-switch-rows">
      {AUTOMATIC_OPERATIONS.map((key) => <div className="asb-provider-switch-row" key={key}>
        <span>{t(operationLabels[key])}</span>
        <div className="asb-provider-operation-controls">
          {draft.capabilities.automaticOperations.includes(key)
            ? <span className="asb-field-help">{t("codex.capability.automatic")}</span>
            : <Button variant="ghost" size="compact" disabled={busy || editor.connection.modelsBusy}
              aria-label={t("codex.capability.restoreAutomaticFor", { operation: t(operationLabels[key]) })}
              onClick={() => setDraft((current) => restoreCodexAutomaticOperation(current, key))}>
              {t("codex.capability.restoreAutomatic")}
            </Button>}
          <Switch checked={draft.capabilities[key]} disabled={busy} label={t(operationLabels[key])}
            onChange={(on) => setDraft((current) => setCodexOperation(current, key, on))} />
        </div>
      </div>)}
    </div>
    {draft.upstream === "chatCompletions" && draft.capabilities.reasoning && <ChatReasoning editor={editor} busy={busy} />}
  </section>;
}
