import { useId, useState } from "react";
import { useI18n } from "../../i18n";
import { claudeOptions } from "../../lib/provider-drafts";
import { providerPreset } from "../../lib/provider-catalog";
import { Button } from "../Button";
import { Input } from "../Input";
import { Table } from "../Table";
import type { ProviderEditorState } from "./useProviderEditor";

export function ClaudeModelCatalog({ editor, busy }: { editor: ProviderEditorState; busy: boolean }) {
  const { t } = useI18n();
  const { draft, setDraft, connection } = editor;
  const [pending, setPending] = useState("");
  const id = useId();
  const preset = providerPreset(draft.baseUrl);
  const models = [...new Set([...(draft.model ? [draft.model] : []),
    ...(draft.modelOptions?.kind === "claude" ? draft.modelOptions.availableModels ?? [] : [])])];
  const suggestions = [...new Map([
    ...(connection.models ?? []).map((model) => [model.id, model.id] as const),
    ...(preset?.protocol === draft.upstreamProtocol ? preset.models.map((model) => [model.id, model.name] as const) : []),
  ])];
  const add = () => {
    const model = pending.trim();
    if (!model) return;
    setDraft((current) => ({ ...current, model: current.model || model,
      modelOptions: claudeOptions(current.modelOptions, { availableModels: [...new Set([
        ...(current.modelOptions?.kind === "claude" ? current.modelOptions.availableModels ?? [] : []), model,
      ])] }),
    }));
    setPending("");
  };
  return <section className="asb-editor-section" aria-label={t("providerSetup.catalog")}>
    <h3 className="asb-section-title">{t("providerSetup.catalog")}</h3>
    <div className="asb-provider-model-toolbar">
      <label className="asb-field"><span>{t("providerSetup.addModel")}</span>
        <Input data-provider-model-entry="" code list={id} value={pending} disabled={busy}
          onChange={(event) => setPending(event.target.value)}
          onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); add(); } }} />
        <datalist id={id}>{suggestions.map(([model, name]) => <option key={model} value={model} label={name} />)}</datalist>
      </label>
      <div className="asb-provider-model-actions">
        <Button variant="secondary" disabled={busy || !pending.trim()} onClick={add}>{t("providerSetup.add")}</Button>
        <Button variant="secondary" disabled={busy || connection.modelsBusy || !connection.baseUrl || !!connection.modelsEndpointError}
          onClick={() => void connection.fetchModels()}>{t("providerEditor.fetchModels")}</Button>
      </div>
    </div>
    {connection.modelsError && <p className="asb-field-error" role="alert">{connection.modelsError}</p>}
    {connection.modelsEndpointError && <p className="asb-field-help">{connection.modelsEndpointError}</p>}
    {models.length > 0 && <Table rows={models} rowKey={(model) => model} ariaLabel={t("providerSetup.catalog")} columns={[
      { key: "model", header: t("providers.editor.availableModels"), render: (model) => <code>{model}</code> },
      { key: "primary", header: t("providerSetup.primary"), render: (model) => model === draft.model ? t("providerSetup.primary")
        : <Button variant="ghost" size="compact" disabled={busy} onClick={() => setDraft((current) => ({ ...current, model }))}>{t("providerSetup.usePrimary")}</Button> },
      { key: "remove", header: t("modelRoutes.edit"), render: (model) => <Button variant="ghost" size="compact" disabled={busy}
        aria-label={t("codex.model.deleteModel", { model })}
        onClick={() => setDraft((current) => ({ ...current,
          model: current.model === model ? null : current.model,
          modelOptions: claudeOptions(current.modelOptions, { availableModels: models.filter((item) => item !== model) }),
        }))}>{t("providerSetup.remove")}</Button> },
    ]} />}
  </section>;
}
