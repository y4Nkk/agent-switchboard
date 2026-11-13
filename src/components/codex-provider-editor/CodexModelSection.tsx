import { useEffect, useState } from "react";
import type { CodexReasoningLevel, LocalizedMessage } from "../../api/client";
import type { TFunction } from "../../i18n";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { CollapsibleSection } from "../Collapsible";
import { Input } from "../Input";
import { Select } from "../Select";
import { Tooltip } from "../Tooltip";
import { PlusIcon, TrashIcon, UpdateIcon } from "../icons";
import {
  REASONING_LEVELS,
  REASONING_LEVEL_LABELS,
  defaultModelLimits,
  emptyCatalogEntry,
  mergeFetchedCatalog,
} from "./draft";
import type { CodexEditorState } from "./useCodexProviderEditor";
import type { EditableCodexCatalogEntry } from "./draft";

interface Props {
  editor: CodexEditorState;
  busy: boolean;
  userConfigModel: string | null;
  userConfigWarnings: LocalizedMessage[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  revealRevision: number;
}

type CatalogUpdate = (patch: Partial<EditableCodexCatalogEntry>) => void;

function modelName(entry: EditableCodexCatalogEntry, t: TFunction): string {
  return entry.id || t("codex.model.fallbackName");
}

function CatalogCapabilities({ entry, capabilities, busy, update, toggleReasoning }: {
  entry: EditableCodexCatalogEntry;
  capabilities: CodexEditorState["draft"]["capabilities"];
  busy: boolean;
  update: CatalogUpdate;
  toggleReasoning: (reasoning: boolean) => void;
}) {
  const { t } = useI18n();
  const model = modelName(entry, t);
  return <>
    <div className="asb-provider-catalog-row-flags" role="group" aria-label={t("codex.model.capabilitiesAria", { model })}>
      <Checkbox label={t("codex.capability.functionTools")} ariaLabel={t("codex.model.functionToolsAria", { model })} checked={entry.functionTools}
        disabled={busy || !capabilities.functionTools}
        onChange={(checked) => update({ functionTools: checked && capabilities.functionTools })} />
      <Checkbox label={t("codex.capability.customTools")} ariaLabel={t("codex.model.customToolsAria", { model })} checked={entry.customTools}
        disabled={busy || !capabilities.customTools}
        onChange={(checked) => update({ customTools: checked && capabilities.customTools })} />
      <Checkbox label={t("codex.capability.toolSearch")} ariaLabel={t("codex.model.toolSearchAria", { model })} checked={entry.toolSearch}
        disabled={busy || !capabilities.toolSearch}
        onChange={(checked) => update({ toolSearch: checked && capabilities.toolSearch })} />
      <Checkbox label={t("codex.capability.reasoning")} ariaLabel={t("codex.model.reasoningAria", { model })} checked={entry.reasoning}
        disabled={busy || !capabilities.reasoning}
        onChange={(checked) => toggleReasoning(checked && capabilities.reasoning)} />
      <Checkbox label={t("codex.model.imageInput")} ariaLabel={`${model} ${t("codex.model.imageInput")}`} checked={entry.images}
        disabled={busy}
        onChange={(checked) => update({ images: checked })} />
      <Checkbox label={t("codex.model.compact")} ariaLabel={t("codex.model.compactAria", { model })} checked={entry.compact}
        disabled={busy || !capabilities.compact}
        onChange={(checked) => update({ compact: checked && capabilities.compact })} />
    </div>
  </>;
}

/** Parses a limit input: a positive integer is stored as-is, anything else
 * (including empty) means "use the model's default". */
function parseLimitInput(raw: string): number | null {
  const parsed = Number(raw.trim());
  return raw.trim() && Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

function CatalogLimits({ entry, busy, defaults, explicit, update, toggleLevel }: {
  entry: EditableCodexCatalogEntry;
  busy: boolean;
  defaults: ReturnType<typeof defaultModelLimits>;
  explicit: boolean;
  update: CatalogUpdate;
  toggleLevel: (level: CodexReasoningLevel) => void;
}) {
  const { t } = useI18n();
  const model = modelName(entry, t);
  return <>
    <div className="asb-provider-field-grid">
      <label className="asb-field">
        <span>{t("codex.model.contextWindow")}</span>
        <Input aria-label={t("codex.model.contextWindowAria", { model })} type="number" min="1" step="1"
          required={explicit} placeholder={explicit ? t("providers.conversion.limitPlaceholder") : t("codex.model.defaultLimit", { value: defaults.contextWindow.toLocaleString("en-US") })}
          value={entry.contextWindow?.toString() ?? ""} disabled={busy}
          onChange={(event) => update({ contextWindow: parseLimitInput(event.target.value) })} />
      </label>
      <label className="asb-field">
        <span>{t("codex.model.maxOutput")}</span>
        <Input aria-label={t("codex.model.maxOutputAria", { model })} type="number" min="1" step="1"
          required={explicit} placeholder={explicit ? t("providers.conversion.limitPlaceholder") : t("codex.model.defaultLimit", { value: defaults.maxOutputTokens.toLocaleString("en-US") })}
          value={entry.maxOutputTokens?.toString() ?? ""} disabled={busy}
          onChange={(event) => update({ maxOutputTokens: parseLimitInput(event.target.value) })} />
      </label>
      <label className="asb-field">
        <span>{t("codex.model.defaultLevel")}</span>
        <Select ariaLabel={t("codex.model.defaultLevelAria", { model })} disabled={busy}
          value={entry.defaultReasoningLevel}
          options={entry.supportedReasoningLevels.map((level) => ({
            value: level, label: t(REASONING_LEVEL_LABELS[level]),
          }))}
          onChange={(value) => update({ defaultReasoningLevel: value as CodexReasoningLevel })} />
      </label>
    </div>
    <div className="asb-provider-catalog-row-levels">
      <span className="asb-catalog-levels-label">{t("codex.model.supportedLevels")}</span>
      <div className="asb-catalog-levels-options" role="group" aria-label={t("codex.model.supportedLevelsAria", { model })}>
        {REASONING_LEVELS.map((level) => (
          <Checkbox key={level} label={t(REASONING_LEVEL_LABELS[level])}
            ariaLabel={t("codex.model.levelAria", { model, level: t(REASONING_LEVEL_LABELS[level]) })}
            checked={entry.supportedReasoningLevels.includes(level)}
            disabled={busy || !entry.reasoning}
            onChange={() => toggleLevel(level)} />
        ))}
      </div>
    </div>
  </>;
}

function CatalogRow({ editor, busy, entry, index }: {
  editor: CodexEditorState;
  busy: boolean;
  entry: EditableCodexCatalogEntry;
  index: number;
}) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  const { capabilities } = draft;
  const defaults = defaultModelLimits(entry.id);
  const update = (patch: Partial<EditableCodexCatalogEntry>) =>
    setDraft((current) => ({
      ...current,
      catalog: current.catalog.map((item, itemIndex) =>
        itemIndex === index ? { ...item, ...patch } : item),
    }));
  const toggleLevel = (level: CodexReasoningLevel) => {
    const levels = entry.supportedReasoningLevels.includes(level)
      ? entry.supportedReasoningLevels.filter((value) => value !== level)
      : [...entry.supportedReasoningLevels, level];
    const ordered = REASONING_LEVELS.filter((value) => levels.includes(value));
    const defaultLevel = ordered.includes(entry.defaultReasoningLevel)
      ? entry.defaultReasoningLevel
      : ordered[0] ?? "none";
    update({ supportedReasoningLevels: ordered, defaultReasoningLevel: defaultLevel });
  };
  const toggleReasoning = (reasoning: boolean) => update(reasoning
    ? { reasoning, defaultReasoningLevel: "medium", supportedReasoningLevels: [...REASONING_LEVELS] }
    : { reasoning, defaultReasoningLevel: "none", supportedReasoningLevels: ["none"] });
  return (
    <div className="asb-provider-catalog-row" aria-label={t("codex.model.rowAria", { model: entry.id || t("codex.model.unnamed") })}>
      <div className="asb-provider-catalog-row-head">
        <label className="asb-field">
          <span>{t("codex.model.idLabel")}</span>
          <Input code aria-label={t("codex.model.idAria", { index: index + 1 })} value={entry.id} required disabled={busy}
            onChange={(event) => update({ id: event.target.value })} />
        </label>
        <Tooltip label={t("codex.model.deleteModel", { model: entry.id || index + 1 })}>
          <Button variant="icon" aria-label={t("codex.model.deleteModel", { model: entry.id || index + 1 })} disabled={busy}
            onClick={() => setDraft((current) => ({
              ...current,
              catalog: current.catalog.filter((_, itemIndex) => itemIndex !== index),
              modelRoutes: current.modelRoutes.filter((route) => route.clientModel !== entry.id),
              defaultModel: current.defaultModel === entry.id ? "" : current.defaultModel,
            }))}>
            <TrashIcon />
          </Button>
        </Tooltip>
      </div>
      <CatalogLimits entry={entry} busy={busy} defaults={defaults} explicit={Boolean(editor.conversion)}
        update={update} toggleLevel={toggleLevel} />
      <CatalogCapabilities entry={entry} capabilities={capabilities} busy={busy}
        update={update} toggleReasoning={toggleReasoning} />
      {entry.reasoning && !capabilities.reasoning && (
        <p className="asb-scope-note asb-warn-text">{t("codex.model.reasoningNotDeclared")}</p>
      )}
    </div>
  );
}

function ModelMapping({ editor, busy, expanded, setExpanded }: { editor: CodexEditorState; busy: boolean; expanded: boolean; setExpanded: (open: boolean) => void }) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  const update = (index: number, clientModel: string, upstreamModel: string) =>
    setDraft((current) => ({
      ...current,
      modelRoutes: current.modelRoutes.map((route, routeIndex) =>
        routeIndex === index ? { clientModel, upstreamModel } : route),
    }));
  return (
    <CollapsibleSection open={expanded} onOpenChange={setExpanded}
      label={t("codex.model.mapping")}
      value={draft.modelRoutes.length > 0 ? t("codex.model.mappingCount", { count: draft.modelRoutes.length }) : t("codex.model.mappingNone")}>
      <p className="asb-scope-note">{t("codex.model.mappingNote")}</p>
      <div className="asb-provider-mapping-list">
        {draft.modelRoutes.map((route, index) => (
          <div className="asb-provider-mapping-row" key={index} aria-label={t("codex.model.mapping")}>
            <Select ariaLabel={t("codex.model.mappingClientAria", { index: index + 1 })} value={route.clientModel} disabled={busy}
              options={draft.catalog.map((entry) => ({ value: entry.id, label: entry.id }))}
              onChange={(value) => update(index, value, route.upstreamModel)} />
            <span className="asb-provider-mapping-arrow" aria-hidden="true">→</span>
            <Input code aria-label={t("codex.model.mappingUpstreamAria", { index: index + 1 })} value={route.upstreamModel}
              disabled={busy} onChange={(event) => update(index, route.clientModel, event.target.value)} />
            <Button variant="danger" aria-label={t("codex.model.mappingDeleteAria", { index: index + 1 })} disabled={busy}
              onClick={() => setDraft((current) => ({
                ...current,
                modelRoutes: current.modelRoutes.filter((_, routeIndex) => routeIndex !== index),
              }))}>
              <TrashIcon />
            </Button>
          </div>
        ))}
      </div>
      <div className="asb-provider-model-actions asb-provider-model-actions-end">
        <Tooltip label={t("codex.model.mappingAdd")}>
          <Button variant="icon" aria-label={t("codex.model.mappingAdd")} disabled={busy || draft.catalog.length === 0}
            onClick={() => setDraft((current) => ({
              ...current,
              modelRoutes: [...current.modelRoutes,
                { clientModel: current.catalog[0]?.id ?? "", upstreamModel: "" }],
            }))}>
            <PlusIcon />
          </Button>
        </Tooltip>
      </div>
    </CollapsibleSection>
  );
}

/** Structured catalog and mapping editor; no raw JSON anywhere. */
export function CodexModelSection({ editor, busy, userConfigModel, userConfigWarnings, open, onOpenChange, revealRevision }: Props) {
  const { t } = useI18n();
  const { draft, setDraft, connection } = editor;
  const [mappingOpen, setMappingOpen] = useState(() => draft.modelRoutes.length > 0);
  useEffect(() => { if (revealRevision > 0) setMappingOpen(true); }, [revealRevision]);
  const fetchIntoCatalog = async () => {
    const models = await connection.fetchModels();
    if (models) setDraft((current) => ({
      ...current,
      catalog: mergeFetchedCatalog(current.catalog, models, current.capabilities).map((entry) =>
        editor.conversion && !current.catalog.some((old) => old.id === entry.id)
          ? { ...entry, contextWindow: null, maxOutputTokens: null, images: false } : entry),
    }));
  };
  return (
    <CollapsibleSection className="asb-editor-section" open={open} onOpenChange={onOpenChange} label={t("providerEditor.catalog")}>
      <div className="asb-editor-section-fields">
        <div className="asb-provider-model-toolbar">
          <div className="asb-provider-model-actions" aria-label={t("codex.model.catalogActions")}>
            <Tooltip label={connection.modelsBusy ? t("codex.model.fetching") : connection.modelsEndpointError ?? t("codex.model.fetch")}>
              <Button variant="icon" aria-label={connection.modelsBusy ? t("codex.model.fetching") : connection.modelsEndpointError ?? t("codex.model.fetch")}
                aria-busy={connection.modelsBusy || undefined}
                disabled={busy || connection.modelsBusy || !connection.baseUrl || !!connection.modelsEndpointError}
                onClick={() => void fetchIntoCatalog()}>
                <UpdateIcon />
              </Button>
            </Tooltip>
            <Tooltip label={t("codex.model.add")}>
              <Button variant="icon" aria-label={t("codex.model.add")} disabled={busy}
                onClick={() => setDraft((current) => ({
                  ...current,
                  catalog: [...current.catalog, { ...emptyCatalogEntry(current.capabilities),
                    ...(editor.conversion ? { images: false } : {}) }],
                }))}>
                <PlusIcon />
              </Button>
            </Tooltip>
          </div>
        </div>
        {userConfigModel && <p className="asb-scope-note">{t("codex.model.userConfigModel", { model: userConfigModel })}</p>}
        {userConfigWarnings.map((warning) => (
          <p key={warning.key} className="asb-scope-note asb-warn-text">{localizedMessageText(warning, t)}</p>
        ))}
        {connection.modelsEndpointError && <span className="asb-warn-text">{connection.modelsEndpointError}</span>}
        {connection.modelsError && connection.modelsError !== connection.modelsEndpointError && <span className="asb-warn-text">{connection.modelsError}</span>}
        <p className="asb-scope-note">{t("codex.model.fetchNote")}</p>
        <div className="asb-provider-catalog">
          {draft.catalog.map((entry, index) => (
            <CatalogRow key={index} editor={editor} busy={busy} entry={entry} index={index} />
          ))}
        </div>
        <ModelMapping editor={editor} busy={busy} expanded={mappingOpen} setExpanded={setMappingOpen} />
      </div>
    </CollapsibleSection>
  );
}
