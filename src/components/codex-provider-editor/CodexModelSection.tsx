import { useId, useState } from "react";
import type { LocalizedMessage } from "../../api/client";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { formatCompactTokenCount } from "../../lib/token-format";
import { catalogModel, providerPreset } from "../../lib/provider-catalog";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { SettingsGroup } from "../SettingsLayout";
import { ToggleChip } from "../ToggleChip";
import { ModelCatalogCard } from "../provider-editor/ModelCatalogCard";
import { ModelPickerDialog, type PickerCandidate } from "../provider-editor/ModelPickerDialog";
import { addCatalogModel, applyCatalogModels, modelSource, removeCatalogModel, withCatalog } from "./catalog";
import { REASONING_LEVELS, REASONING_LEVEL_LABELS, type EditableCodexCatalogEntry } from "./draft";
import type { CodexEditorState } from "./useCodexProviderEditor";

const capabilityFlags = [
  ["functionTools", "codex.capability.functionTools"], ["customTools", "codex.capability.customTools"],
  ["toolSearch", "codex.capability.toolSearch"], ["reasoning", "codex.capability.reasoning"],
  ["images", "codex.model.imageInput"], ["compact", "codex.model.compact"],
] as const;

/** Preset chips align exactly with catalog snapshot values, so auto-filled
 * limits light a chip up; other values stay freely typable. */
const contextPresets = [
  [128_000, "128K"], [200_000, "200K"], [256_000, "256K"], [400_000, "400K"], [1_000_000, "1M"],
] as const;
const outputPresets = [
  [8_192, "8K"], [16_384, "16K"], [32_768, "32K"], [65_536, "64K"], [128_000, "128K"],
] as const;

function ModelCard({ entry, primary, busy, onChange, onPrimary, onRemove, sourceLabel }: {
  entry: EditableCodexCatalogEntry; primary: boolean; busy: boolean;
  onChange: (patch: Partial<EditableCodexCatalogEntry>) => void;
  onPrimary: () => void; onRemove: () => void; sourceLabel: string;
}) {
  const { t } = useI18n();
  const levels = entry.supportedReasoningLevels;
  const toggleLevel = (level: (typeof REASONING_LEVELS)[number]) => {
    const next = levels.includes(level) ? levels.filter((item) => item !== level) : [...levels, level];
    const ordered = REASONING_LEVELS.filter((item) => next.includes(item));
    const supported = ordered.length ? ordered : ["none" as const];
    onChange({
      supportedReasoningLevels: supported,
      defaultReasoningLevel: supported.includes(entry.defaultReasoningLevel)
        ? entry.defaultReasoningLevel
        : supported.includes("medium") ? "medium" : supported[0],
    });
  };
  const limitField = (key: "contextWindow" | "maxOutputTokens", labelKey: "codex.model.contextWindow" | "codex.model.maxOutput",
      presets: typeof contextPresets | typeof outputPresets) => {
    const value = entry[key];
    return <label className="asb-field"><span>{t(labelKey)}</span>
      <Input type="number" min={1} step={1} required code disabled={busy} value={value ?? ""}
        placeholder={t("providerSetup.unconfirmed")}
        aria-label={t(key === "contextWindow" ? "codex.model.contextWindowAria" : "codex.model.maxOutputAria", { model: entry.id })}
        onChange={(event) => onChange({ [key]: event.target.value ? Number(event.target.value) : null })} />
      <div className="asb-chip-row">
        {presets.map(([tokens, label]) => <ToggleChip key={label} pressed={value === tokens} label={label}
          ariaLabel={`${entry.id} ${t(labelKey)} ${label}`} disabled={busy}
          onClick={() => onChange({ [key]: tokens })} />)}
      </div>
    </label>;
  };
  return <ModelCatalogCard app="codex" model={entry.id} primary={primary} busy={busy} sourceLabel={sourceLabel}
    onPrimary={onPrimary} onRemove={onRemove}>
    <div className="asb-provider-model-limits">
      {limitField("contextWindow", "codex.model.contextWindow", contextPresets)}
      {limitField("maxOutputTokens", "codex.model.maxOutput", outputPresets)}
    </div>
    <div className="asb-provider-model-meta">
      <div className="asb-chip-row" role="group" aria-label={t("providerSetup.capabilities")}>
        {capabilityFlags.map(([key, labelKey]) => <ToggleChip key={key} pressed={entry[key]} label={t(labelKey)}
          ariaLabel={`${entry.id} ${t(labelKey)}`} disabled={busy}
          onClick={() => onChange(key === "reasoning" && entry[key]
            ? { reasoning: false, supportedReasoningLevels: ["none"], defaultReasoningLevel: "none" }
            : { [key]: !entry[key] })} />)}
      </div>
      {entry.reasoning && <>
        <div className="asb-chip-row" role="group" aria-label={t("providerSetup.levels")}>
          {REASONING_LEVELS.map((level) => <ToggleChip key={level} pressed={levels.includes(level)}
            label={t(REASONING_LEVEL_LABELS[level])} ariaLabel={`${entry.id} ${t(REASONING_LEVEL_LABELS[level])}`}
            disabled={busy} onClick={() => toggleLevel(level)} />)}
        </div>
        <div className="asb-setting-input-row"><span>{t("codex.model.defaultLevel")}</span>
          <Select ariaLabel={`${entry.id} ${t("codex.model.defaultLevel")}`} value={entry.defaultReasoningLevel} disabled={busy}
            options={levels.map((level) => ({ value: level, label: t(REASONING_LEVEL_LABELS[level]) }))}
            onChange={(defaultReasoningLevel) => onChange({ defaultReasoningLevel: defaultReasoningLevel as EditableCodexCatalogEntry["defaultReasoningLevel"] })} />
        </div>
      </>}
    </div>
  </ModelCatalogCard>;
}

export function CodexModelSection({ editor, busy, userConfigModel, userConfigWarnings }: {
  editor: CodexEditorState; busy: boolean; userConfigModel: string | null; userConfigWarnings: LocalizedMessage[];
}) {
  const { t } = useI18n();
  const { draft, setDraft, connection, fetchModels } = editor;
  const [pending, setPending] = useState("");
  const [picking, setPicking] = useState(false);
  const listId = useId();
  const known = providerPreset(draft.endpoint, draft.upstream);
  const suggestions = connection.models?.map(({ id }) => [id, catalogModel(draft.endpoint, draft.upstream, id)?.name ?? id] as const)
    ?? known?.models.map(({ id, name }) => [id, name] as const) ?? [];
  const candidateIds = new Set([...suggestions.map(([id]) => id), ...draft.catalog.map(({ id }) => id)]);
  const candidates: PickerCandidate[] = [...candidateIds].map((id) => {
    const model = catalogModel(draft.endpoint, draft.upstream, id);
    if (!model) return { id };
    const hint = [model.contextWindow ? `${formatCompactTokenCount(model.contextWindow)} ${t("codex.model.contextWindow")}` : null,
      model.reasoning ? t("codex.capability.reasoning") : null, model.images ? t("codex.model.imageInput") : null]
      .filter(Boolean).join(" · ");
    return hint ? { id, hint } : { id };
  });
  const update = (id: string, patch: Partial<EditableCodexCatalogEntry>) => setDraft((current) =>
    withCatalog(current, current.catalog.map((entry) => entry.id === id ? { ...entry, ...patch } : entry)));
  const add = () => {
    const id = pending.trim();
    if (!id) return;
    setDraft((current) => addCatalogModel(current, id));
    setPending("");
  };
  return <SettingsGroup title={t("providerSetup.catalog")} surface="plain">
    <div className="asb-editor-section-fields">
      <div className="asb-provider-model-toolbar">
        <div className="asb-provider-model-actions">
          <Button variant="secondary" disabled={busy || connection.modelsBusy || !connection.baseUrl || !!connection.modelsEndpointError}
            onClick={() => void fetchModels().then((models) => {
              if (models !== null) setPicking(true);
            })}>{t("providerEditor.fetchModels")}</Button>
        </div>
        <label className="asb-field"><span>{t("providerSetup.addModel")}</span>
          <Input data-provider-model-entry="" code value={pending} list={listId} disabled={busy}
            onChange={(event) => setPending(event.target.value)}
            onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); add(); } }} />
          <datalist id={listId}>{suggestions.map(([id, name]) => <option key={id} value={id} label={name} />)}</datalist>
        </label>
        <div className="asb-provider-model-actions">
          <Button variant="secondary" disabled={busy || !pending.trim()} onClick={add}>{t("providerSetup.add")}</Button>
        </div>
      </div>
      {connection.modelsError && <p className="asb-field-error" role="alert">{connection.modelsError}</p>}
      {connection.modelsEndpointError && <p className="asb-field-help">{connection.modelsEndpointError}</p>}
      {draft.catalog.length ? draft.catalog.map((entry) => <ModelCard key={entry.id} entry={entry} busy={busy}
          primary={entry.id === draft.defaultModel} sourceLabel={t(`providerSetup.${modelSource(draft, entry)}`)}
          onChange={(patch) => update(entry.id, patch)}
          onPrimary={() => setDraft((current) => ({ ...current, defaultModel: entry.id }))}
          onRemove={() => setDraft((current) => removeCatalogModel(current, entry.id))} />)
        : <p className="asb-field-help">{t("providerSetup.noModels")}</p>}
      {userConfigModel && <p className="asb-field-help">{t("codex.model.userConfigModel", { model: userConfigModel })}</p>}
      {userConfigWarnings.map((warning) => <p key={warning.key} className="asb-warn-text">{localizedMessageText(warning, t)}</p>)}
    </div>
    {picking && <ModelPickerDialog title={t("providerSetup.pickerTitle")} candidates={candidates} busy={busy}
      selected={new Set(draft.catalog.map(({ id }) => id))}
      onApply={(ids) => setDraft((current) => applyCatalogModels(current, ids))} onClose={() => setPicking(false)} />}
  </SettingsGroup>;
}
