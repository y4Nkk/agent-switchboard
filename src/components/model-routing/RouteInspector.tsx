import type { CodexProviderRecord, ProviderDraft } from "../../api/client";
import { useI18n } from "../../i18n";
import { Checkbox } from "../Checkbox";
import { Input } from "../Input";
import { ModelPicker } from "../ModelPicker";
import { RadioOption } from "../RadioOption";
import { Select } from "../Select";
import { claudeOptions } from "../../lib/provider-drafts";
import type { RouteDocument, RouteRole, RouteRecord } from "./model";

interface Props {
  document: RouteDocument;
  role: RouteRole;
  codex: CodexProviderRecord[];
  sources: RouteRecord[];
  disabled: boolean;
  onChange: (document: RouteDocument) => void;
  onSelectProfile: (id: string) => void;
}

function SourceField({ document, sources, onSelectProfile, disabled }: Props) {
  const { t } = useI18n();
  return <label className="asb-field"><span>{t("modelRoutes.provider")}</span>
    <Select value={document.record.profile.id} options={sources.map(({ record }) => ({ value: record.profile.id, label: record.profile.name }))}
      ariaLabel={t("modelRoutes.provider")} disabled={disabled} onChange={onSelectProfile} />
  </label>;
}

function CatalogModelField({ value, label, models, provider, disabled, onSelect }: {
  value: string; label: string; models: { id: string }[]; provider: string;
  disabled: boolean; onSelect: (model: string) => void;
}) {
  return <div className="asb-field"><span>{label}</span><div className="asb-input-with-picker">
    <Input code readOnly value={value} aria-label={label} disabled={disabled} />
    <ModelPicker current={value || null} ariaLabel={label} disabled={disabled}
      models={models.map(({ id }) => ({ value: id, label: id, group: provider }))} onSelect={onSelect} />
  </div></div>;
}

function CodexInspector(props: Props & { document: Extract<RouteDocument, { kind: "codex" }> }) {
  const { t } = useI18n();
  const { document, role, codex, disabled, onChange } = props;
  const route = document.draft.subagentRoute;
  const target = route?.profileId === document.record.profile.id ? document.draft
    : codex.find(({ profile }) => profile.id === route?.profileId)?.profile;
  const change = (patch: Partial<typeof document.draft>) => onChange({ ...document, draft: { ...document.draft, ...patch } });
  return <>
    {role === "main" ? <>
      <SourceField {...props} />
      <CatalogModelField value={document.draft.defaultModel} label={t("modelRoutes.model")} disabled={disabled}
        models={document.draft.catalog} provider={document.draft.name} onSelect={(defaultModel) => change({ defaultModel })} />
    </> : <>
      <div className="asb-segments" role="radiogroup" aria-label={t("modelRoutes.subagentMode")}>
        <RadioOption name="model-route-mode" checked={!route} disabled={disabled} label={t("modelRoutes.automatic")}
          onChange={() => change({ subagentRoute: null })} />
        <RadioOption name="model-route-mode" checked={!!route} disabled={disabled} label={t("modelRoutes.specify")}
          onChange={() => change({ subagentRoute: { profileId: "", model: "" } })} />
      </div>
      {route && <>
        <label className="asb-field"><span>{t("modelRoutes.targetProvider")}</span>
          <Select value={route.profileId || null} placeholder={t("modelRoutes.chooseProvider")}
            options={codex.filter(({ profile }) => !profile.connection?.authBinding)
              .map(({ profile }) => ({ value: profile.id, label: profile.name }))}
            ariaLabel={t("modelRoutes.targetProvider")} disabled={disabled}
            onChange={(profileId) => change({ subagentRoute: { profileId, model: "" } })} />
        </label>
        {target?.catalog.length ? <CatalogModelField value={route.model} label={t("modelRoutes.targetModel")} disabled={disabled}
          models={target.catalog} provider={target.name} onSelect={(model) => change({ subagentRoute: { ...route, model } })} />
          : <p className="asb-field-help">{t("modelRoutes.chooseProviderFirst")}</p>}
      </>}
      <p className="asb-field-help">{t("modelRoutes.subagentHelp")}</p>
    </>}
  </>;
}

function ClaudeInspector(props: Props & { document: Extract<RouteDocument, { kind: "profile" }> }) {
  const { t } = useI18n();
  const { document, role, disabled, onChange } = props;
  const draft = document.draft;
  if (draft.routeMode === "official") return <><SourceField {...props} /><p className="asb-field-help">{t("modelRoutes.official")}</p></>;
  if (role === "subagent") return null;
  const options = draft.modelOptions?.kind === "claude" ? draft.modelOptions : null;
  const value = role === "main" ? draft.model : options?.[`${role}Model`];
  const change = (next: ProviderDraft) => onChange({ ...document, draft: next });
  const modelChange = (value: string) => {
    const model = value.trim();
    change(role === "main"
      ? { ...draft, model: model || null, modelOptions: claudeOptions(draft.modelOptions, !model ? { primaryOneM: false } : {}) }
      : { ...draft, modelOptions: claudeOptions(draft.modelOptions, { [`${role}Model`]: model || null,
        ...(!model ? { [`${role}OneM`]: false } : {}) }) });
  };
  const flag = role === "main" ? "primaryOneM" : role === "sonnet" ? "sonnetOneM"
    : role === "opus" ? "opusOneM" : role === "fable" ? "fableOneM" : null;
  const models = [...new Set([draft.model, ...(options?.availableModels ?? []), options?.haikuModel, options?.sonnetModel, options?.opusModel, options?.fableModel])]
    .filter((id): id is string => !!id);
  return <>
    {role === "main" ? <SourceField {...props} /> : <p className="asb-field-help">{t("modelRoutes.sameProvider", { name: draft.name })}</p>}
    <div className="asb-field"><span>{t("modelRoutes.model")}</span><div className="asb-input-with-picker">
      <Input code value={value ?? ""} aria-label={t("modelRoutes.model")} disabled={disabled}
        placeholder={t("modelRoutes.clientDefault")} onChange={(event) => modelChange(event.target.value)} />
      {models.length > 0 && <ModelPicker models={models.map((id) => ({ value: id, label: id, group: draft.name }))}
        current={value ?? null} ariaLabel={t("modelRoutes.pickSavedModel")} disabled={disabled} onSelect={modelChange} />}
      {flag && <Checkbox label="1M" ariaLabel={t("modelRoutes.oneM")} checked={options?.[flag] ?? false}
        disabled={disabled || !value?.trim()} onChange={(enabled) => change({ ...draft,
          modelOptions: claudeOptions(draft.modelOptions, { [flag]: enabled }) })} />}
    </div></div>
  </>;
}

export function RouteInspector(props: Props) {
  return props.document.kind === "codex"
    ? <CodexInspector {...props} document={props.document} /> : <ClaudeInspector {...props} document={props.document} />;
}
