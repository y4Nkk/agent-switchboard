import { useId, useState } from "react";
import { useI18n } from "../../i18n";
import { claudeOptions } from "../../lib/provider-drafts";
import { catalogModel, providerPreset } from "../../lib/provider-catalog";
import { formatCompactTokenCount } from "../../lib/token-format";
import { Button } from "../Button";
import { Input } from "../Input";
import { Table } from "../Table";
import { ModelPickerDialog, type PickerCandidate } from "./ModelPickerDialog";
import type { ProviderEditorState } from "./useProviderEditor";

export function ClaudeModelCatalog({ editor, busy }: { editor: ProviderEditorState; busy: boolean }) {
  const { t } = useI18n();
  const { draft, setDraft, connection } = editor;
  const [pending, setPending] = useState("");
  const [picking, setPicking] = useState(false);
  const id = useId();
  const preset = providerPreset(draft.baseUrl);
  const models = [...new Set([...(draft.model ? [draft.model] : []),
    ...(draft.modelOptions?.kind === "claude" ? draft.modelOptions.availableModels ?? [] : [])])];
  const suggestions = [...new Map([
    ...(connection.models ?? []).map((model) => [model.id, model.id] as const),
    ...(preset?.protocol === draft.upstreamProtocol ? preset.models.map((model) => [model.id, model.name] as const) : []),
  ])];
  const candidates: PickerCandidate[] = suggestions.map(([model]) => {
    const known = draft.upstreamProtocol ? catalogModel(draft.baseUrl, draft.upstreamProtocol, model) : undefined;
    if (!known) return { id: model };
    const hint = [known.contextWindow ? `${formatCompactTokenCount(known.contextWindow)} ${t("codex.model.contextWindow")}` : null,
      known.reasoning ? t("codex.capability.reasoning") : null, known.images ? t("codex.model.imageInput") : null]
      .filter(Boolean).join(" · ");
    return hint ? { id: model, hint } : { id: model };
  });
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
      <div className="asb-provider-model-actions">
        <Button variant="secondary" disabled={busy || connection.modelsBusy || !connection.baseUrl || !!connection.modelsEndpointError}
          onClick={() => void connection.fetchModels().then(() => setPicking(true))}>{t("providerEditor.fetchModels")}</Button>
      </div>
      <label className="asb-field"><span>{t("providerSetup.addModel")}</span>
        <Input data-provider-model-entry="" code list={id} value={pending} disabled={busy}
          onChange={(event) => setPending(event.target.value)}
          onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); add(); } }} />
        <datalist id={id}>{suggestions.map(([model, name]) => <option key={model} value={model} label={name} />)}</datalist>
      </label>
      <div className="asb-provider-model-actions">
        <Button variant="secondary" disabled={busy || !pending.trim()} onClick={add}>{t("providerSetup.add")}</Button>
      </div>
    </div>
    {connection.modelsError && <p className="asb-field-error" role="alert">{connection.modelsError}</p>}
    {connection.modelsEndpointError && <p className="asb-field-help">{connection.modelsEndpointError}</p>}
    {models.length > 0 && <Table rows={models} rowKey={(model) => model} ariaLabel={t("providerSetup.catalog")} columns={[
      { key: "model", header: t("providers.editor.availableModels"), render: (model) => <code>{model}</code> },
      { key: "primary", header: t("providerSetup.primary"), render: (model) => model === draft.model
        ? <span className="asb-provider-primary-pill">{t("providerSetup.primary")}</span>
        : <Button variant="ghost" size="compact" disabled={busy} onClick={() => setDraft((current) => ({ ...current, model }))}>{t("providerSetup.usePrimary")}</Button> },
      { key: "remove", header: t("providerSetup.actions"), render: (model) => <Button variant="dangerText" size="compact" disabled={busy}
        aria-label={t("codex.model.deleteModel", { model })}
        onClick={() => setDraft((current) => ({ ...current,
          model: current.model === model ? null : current.model,
          modelOptions: claudeOptions(current.modelOptions, { availableModels: models.filter((item) => item !== model) }),
        }))}>{t("providerSetup.remove")}</Button> },
    ]} />}
    {picking && <ModelPickerDialog title={t("providerSetup.pickerTitle")} candidates={candidates} busy={busy}
      selected={new Set(models)}
      onApply={(ids) => setDraft((current) => ({ ...current,
        model: current.model && ids.has(current.model) ? current.model : ids.size ? [...ids][0] : null,
        modelOptions: claudeOptions(current.modelOptions, { availableModels: [...ids] }),
      }))} onClose={() => setPicking(false)} />}
  </section>;
}
