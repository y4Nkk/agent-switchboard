import type { ClaudeModelRoute } from "../../api/client";
import { useI18n } from "../../i18n";
import { claudeOptions } from "../../lib/provider-drafts";
import { Checkbox } from "../Checkbox";
import { Input } from "../Input";
import { ModelPicker } from "../ModelPicker";
import { RadioOption } from "../RadioOption";
import { Select } from "../Select";
import { claudeModels, claudeTargetIssue, type RouteDocument } from "./model";
import type { RouteInspectorProps } from "./RouteInspector";

type Props = RouteInspectorProps & { document: Extract<RouteDocument, { kind: "profile" }> };

export function ClaudeRouteInspector({ document, role, sources, disabled, onChange }: Props) {
  const { t } = useI18n();
  const draft = document.draft;
  if (draft.routeMode === "official") return <p className="asb-field-help">{t("modelRoutes.official")}</p>;
  const options = claudeOptions(draft.modelOptions, {});
  if (options.kind !== "claude") return null;
  const route = role === "main" ? null : options.routes[role];
  const selected = route?.mode === "provider" ? sources.find((source) => source.kind === "profile" && source.record.profile.id === route.profileId) : null;
  const target = route?.mode === "provider" ? route.profileId === document.profileId ? draft : selected?.kind === "profile" ? selected.record.profile : undefined : draft;
  const models = [...new Set([
    ...(target ? claudeModels(target, route?.mode === "provider" ? route.profileId : document.profileId) : []),
  ])];
  const value = role === "main" ? draft.model ?? "" : route?.model ?? "";
  const changeRoute = (next: ClaudeModelRoute | null) => {
    if (role === "main") return;
    onChange({ ...document, draft: { ...draft, modelOptions: { ...options, routes: { ...options.routes, [role]: next } } } });
  };
  const changeModel = (model: string) => {
    if (role === "main") onChange({ ...document, draft: { ...draft, model: model.trim() || null,
      modelOptions: { ...options, primaryOneM: !!model.trim() && options.primaryOneM } } });
    else changeRoute(model.trim() ? { ...(route ?? { mode: "primary", oneM: false }), model: model.trim() }
      : route ? { ...route, model: "", oneM: false } : null);
  };
  const native = !!draft.connection?.claudeNative;
  return <>
    <p className="asb-field-help">{t("modelRoutes.mainOwner", { name: draft.name })}</p>
    {role !== "main" && <div className="asb-segments" role="radiogroup" aria-label={t("modelRoutes.routeMode")}>
      <RadioOption name="claude-route-mode" checked={!route} disabled={disabled}
        label={t(role === "fable" && options.routes.opus ? "modelRoutes.inheritOpus" : "modelRoutes.purposeDefault")}
        onChange={() => changeRoute(null)} />
      <RadioOption name="claude-route-mode" checked={route?.mode === "primary"} disabled={disabled}
        label={t("modelRoutes.primaryModelMode")} onChange={() => changeRoute({ mode: "primary", model: "", oneM: false })} />
      <RadioOption name="claude-route-mode" checked={route?.mode === "provider"} disabled={disabled || native}
        label={t("modelRoutes.specify")} onChange={() => changeRoute({ mode: "provider", profileId: "", model: "", oneM: false })} />
    </div>}
    {native && role !== "main" && <p className="asb-field-help">{t("modelRoutes.unsupportedTarget")}</p>}
    {route?.mode === "provider" && <>
      <label className="asb-field"><span>{t("modelRoutes.targetProvider")}</span>
        <Select value={route.profileId || null} placeholder={t("modelRoutes.chooseProvider")}
          options={[
            ...(!claudeTargetIssue(draft) ? [{ value: document.profileId,
              label: t("providerEditor.selfTarget", { name: draft.name }) }] : []),
            ...sources.filter((source) => source.kind === "profile" && source.record.profile.id !== document.profileId
              && !claudeTargetIssue(source.record.profile))
              .map(({ record }) => ({ value: record.profile.id, label: record.profile.name })),
          ]}
          ariaLabel={t("modelRoutes.targetProvider")} disabled={disabled}
          onChange={(profileId) => changeRoute({ ...route, profileId, model: "", oneM: false })} />
      </label>
      <p className="asb-field-help">{t("modelRoutes.subagentHelp")}</p>
    </>}
    <div className="asb-field"><span>{t("modelRoutes.model")}</span><div className="asb-input-with-picker">
      <Input code value={value} readOnly={route?.mode === "provider"} aria-label={t("modelRoutes.model")}
        disabled={disabled} placeholder={t("modelRoutes.clientDefault")} onChange={(event) => changeModel(event.target.value)} />
      {models.length > 0 && <ModelPicker models={models.map((model) => ({ value: model, label: model, group: target?.name ?? "" }))}
        current={value || null} ariaLabel={t("modelRoutes.pickSavedModel")} disabled={disabled} onSelect={changeModel} />}
      <Checkbox label="1M" ariaLabel={t("modelRoutes.oneM")}
        checked={role === "main" ? options.primaryOneM : route?.oneM ?? false} disabled={disabled || !value.trim()}
        onChange={(oneM) => role === "main"
          ? onChange({ ...document, draft: { ...draft, modelOptions: { ...options, primaryOneM: oneM } } })
          : route && changeRoute({ ...route, oneM })} />
    </div></div>
    {!route && role !== "main" && <p className="asb-field-help">{t("modelRoutes.followHelp")}</p>}
  </>;
}
