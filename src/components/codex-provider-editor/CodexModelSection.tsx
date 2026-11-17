import { useId, useState } from "react";
import type { LocalizedMessage } from "../../api/client";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { providerPreset } from "../../lib/provider-catalog";
import { Button } from "../Button";
import { Input } from "../Input";
import { Table } from "../Table";
import { ProviderValue } from "../provider-editor/ProviderValue";
import { addCatalogModel, modelSource, withCatalog } from "./catalog";
import { CodexCatalogDetails } from "./CodexCatalogDetails";
import { CodexModelMapping } from "./CodexModelMapping";
import type { EditableCodexCatalogEntry } from "./draft";
import type { CodexEditorState } from "./useCodexProviderEditor";

function useModelCatalog(editor: CodexEditorState) {
  const { draft, setDraft, connection } = editor;
  const [pending, setPending] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const listId = useId();
  const known = providerPreset(draft.endpoint);
  const suggestions = [...new Map([
    ...(connection.models ?? []).map(({ id }) => [id, id] as const),
    ...(known?.protocol === draft.upstream ? known.models.map(({ id, name }) => [id, name] as const) : []),
  ])];
  const update = (id: string, patch: Partial<EditableCodexCatalogEntry>) => setDraft((current) =>
    withCatalog(current, current.catalog.map((entry) => entry.id === id ? { ...entry, ...patch } : entry)));
  const add = () => {
    const id = pending.trim();
    if (!id) return;
    setDraft((current) => addCatalogModel(current, id));
    if (!known || known.protocol !== draft.upstream || !known.models.some((model) => model.id === id)) setSelected(id);
    setPending("");
  };
  const remove = (id: string) => {
    setDraft((current) => withCatalog({ ...current,
      defaultModel: current.defaultModel === id ? "" : current.defaultModel,
      modelRoutes: current.modelRoutes.filter(({ clientModel }) => clientModel !== id),
    }, current.catalog.filter((entry) => entry.id !== id)));
    if (selected === id) setSelected(null);
  };
  const entry = draft.catalog.find(({ id }) => id === selected);
  return { draft, setDraft, connection, pending, setPending, listId, suggestions, update, add, remove, entry, setSelected };
}

export function CodexModelSection({ editor, busy, userConfigModel, userConfigWarnings }: {
  editor: CodexEditorState; busy: boolean; userConfigModel: string | null; userConfigWarnings: LocalizedMessage[];
}) {
  const { t } = useI18n();
  const { draft, setDraft, connection, pending, setPending, listId, suggestions, update, add, remove, entry, setSelected } = useModelCatalog(editor);
  const limit = (row: EditableCodexCatalogEntry, key: "contextWindow" | "maxOutputTokens") =>
    <ProviderValue label={t(key === "contextWindow" ? "codex.model.contextWindowAria" : "codex.model.maxOutputAria", { model: row.id })} disabled={busy} required numeric min={1}
      value={row[key]?.toString() ?? ""} onChange={(raw) => update(row.id, { [key]: raw ? Number(raw) : null })} />;
  return <section className="asb-editor-section" aria-label={t("providerSetup.catalog")}>
    <h3 className="asb-section-title">{t("providerSetup.catalog")}</h3>
    <p className="asb-field-help">{t("providerSetup.catalogHelp")}</p>
    <div className="asb-editor-section-fields">
      <div className="asb-provider-model-toolbar">
        <label className="asb-field"><span>{t("providerSetup.addModel")}</span>
          <Input data-provider-model-entry="" code value={pending} list={listId} disabled={busy}
            onChange={(event) => setPending(event.target.value)}
            onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); add(); } }} />
          <datalist id={listId}>{suggestions.map(([id, name]) => <option key={id} value={id} label={name} />)}</datalist>
        </label>
        <div className="asb-provider-model-actions">
          <Button variant="secondary" disabled={busy || !pending.trim()} onClick={add}>{t("providerSetup.add")}</Button>
          <Button variant="secondary" disabled={busy || connection.modelsBusy || !connection.baseUrl || !!connection.modelsEndpointError}
            onClick={() => void connection.fetchModels()}>{t("providerEditor.fetchModels")}</Button>
        </div>
      </div>
      {connection.modelsError && <p className="asb-field-error" role="alert">{connection.modelsError}</p>}
      {connection.modelsEndpointError && <p className="asb-field-help">{connection.modelsEndpointError}</p>}
      {draft.catalog.length ? <Table className="asb-provider-catalog-table" ariaLabel={t("providerSetup.catalog")} rows={draft.catalog} rowKey={({ id }) => id}
        columns={[
          { key: "model", header: t("codex.model.idLabel"), render: (row) => <div><code>{row.id}</code>
            {row.id === draft.defaultModel && <p className="asb-field-help">{t("providerSetup.primary")}</p>}</div> },
          { key: "context", header: t("codex.model.contextWindow"), render: (row) => limit(row, "contextWindow") },
          { key: "output", header: t("codex.model.maxOutput"), render: (row) => limit(row, "maxOutputTokens") },
          { key: "capabilities", header: t("providerSetup.capabilities"), render: (row) => <span>
            {[row.functionTools && t("codex.capability.functionTools"), row.reasoning && row.supportedReasoningLevels.join(", "),
              row.images && t("codex.model.imageInput"), row.customTools && t("codex.capability.customTools"),
              row.toolSearch && t("codex.capability.toolSearch"), row.compact && t("codex.model.compact")].filter(Boolean).join(" · ") || t("providerSetup.disabled")}</span> },
          { key: "source", header: t("providerSetup.source"), render: (row) => t(`providerSetup.${modelSource(draft, row)}`) },
          { key: "actions", header: t("modelRoutes.edit"), render: (row) => <div className="asb-provider-model-actions">
            <Button variant="ghost" size="compact" disabled={busy || row.id === draft.defaultModel}
              onClick={() => setDraft((current) => ({ ...current, defaultModel: row.id }))}>{t("providerSetup.usePrimary")}</Button>
            <Button variant="ghost" size="compact" disabled={busy} onClick={() => setSelected(row.id)}>{t("providerSetup.edit")}</Button>
            <Button variant="ghost" size="compact" disabled={busy} aria-label={t("codex.model.deleteModel", { model: row.id })}
              onClick={() => remove(row.id)}>{t("providerSetup.remove")}</Button>
          </div> },
        ]} /> : <p className="asb-field-help">{t("providerSetup.noModels")}</p>}
      {entry && <CodexCatalogDetails key={entry.id} entry={entry} busy={busy} onChange={(patch) => update(entry.id, patch)} />}
      <CodexModelMapping editor={editor} busy={busy} />
      {userConfigModel && <p className="asb-field-help">{t("codex.model.userConfigModel", { model: userConfigModel })}</p>}
      {userConfigWarnings.map((warning) => <p key={warning.key} className="asb-warn-text">{localizedMessageText(warning, t)}</p>)}
    </div>
  </section>;
}
