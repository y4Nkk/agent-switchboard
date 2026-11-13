import type { CodexProviderRecord } from "../../api/client";
import { useI18n } from "../../i18n";
import { Input } from "../Input";
import { ModelPicker } from "../ModelPicker";
import { RadioOption } from "../RadioOption";
import { Select } from "../Select";
import { ClaudeRouteInspector } from "./ClaudeRouteInspector";
import type { RouteDocument, RouteRole, RouteRecord } from "./model";

export interface RouteInspectorProps {
  document: RouteDocument;
  role: RouteRole;
  codex: CodexProviderRecord[];
  sources: RouteRecord[];
  disabled: boolean;
  onChange: (document: RouteDocument) => void;
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

function CodexInspector(props: RouteInspectorProps & { document: Extract<RouteDocument, { kind: "codex" }> }) {
  const { t } = useI18n();
  const { document, role, codex, disabled, onChange } = props;
  const route = document.draft.subagentRoute;
  const target = route?.profileId === document.profileId ? document.draft
    : codex.find(({ profile }) => profile.id === route?.profileId)?.profile;
  const change = (patch: Partial<typeof document.draft>) => onChange({ ...document, draft: { ...document.draft, ...patch } });
  return <>
    {role === "main" ? <>
      <p className="asb-field-help">{t("modelRoutes.mainOwner", { name: document.draft.name })}</p>
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
            options={[
              ...(!document.draft.connection?.authBinding ? [{ value: document.profileId,
                label: t("providerEditor.selfTarget", { name: document.draft.name }) }] : []),
              ...codex.filter(({ profile }) => profile.id !== document.profileId && !profile.connection?.authBinding)
                .map(({ profile }) => ({ value: profile.id, label: profile.name })),
            ]}
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

export function RouteInspector(props: RouteInspectorProps) {
  return props.document.kind === "codex"
    ? <CodexInspector {...props} document={props.document} /> : <ClaudeRouteInspector {...props} document={props.document} />;
}
