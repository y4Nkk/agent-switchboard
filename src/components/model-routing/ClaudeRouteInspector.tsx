import type { ClaudeModelRoute } from "../../api/client";
import { useI18n } from "../../i18n";
import { claudeOptions } from "../../lib/provider-drafts";
import { Input } from "../Input";
import { ModelPicker } from "../ModelPicker";
import { Select } from "../Select";
import { claudeModels, claudeTargetIssue, type RouteDocument } from "./model";
import type { RouteInspectorProps } from "./RouteInspector";

type Props = RouteInspectorProps & { document: Extract<RouteDocument, { kind: "profile" }> };

function claudeRouteState({ document, role, sources, onChange }: Props) {
  const draft = document.draft;
  const options = claudeOptions(draft.modelOptions, {});
  if (options.kind !== "claude") throw new Error("Claude routing requires Claude model options");
  const route = role === "main" ? null : options.routes[role];
  const selected = route?.mode === "provider" ? sources.find((source) => source.kind === "profile" && source.record.profile.id === route.profileId) : null;
  const target = route?.mode === "provider" ? route.profileId === document.profileId ? draft
    : selected?.kind === "profile" ? selected.record.profile : undefined : draft;
  const models = target ? claudeModels(target, route?.mode === "provider" ? route.profileId : document.profileId) : [];
  const value = role === "main" ? draft.model ?? "" : route?.model ?? "";
  const changeRoute = (next: ClaudeModelRoute | null) => {
    if (role !== "main") onChange({ ...document, draft: { ...draft,
      modelOptions: { ...options, routes: { ...options.routes, [role]: next } },
    } });
  };
  const changeModel = (model: string) => changeRoute(model.trim()
    ? { ...(route ?? { mode: "primary", oneM: false }), model: model.trim() }
    : route ? { ...route, model: "", oneM: false } : null);
  const changeOneM = (oneM: boolean) => role === "main"
    ? onChange({ ...document, draft: { ...draft, modelOptions: { ...options, primaryOneM: oneM } } })
    : route && changeRoute({ ...route, oneM });
  return { draft, options, route, target, models, value, changeRoute, changeModel, changeOneM };
}

export function ClaudeRouteInspector(props: Props) {
  const { t } = useI18n();
  const { document, role, sources, disabled } = props;
  const { draft, options, route, target, models, value, changeRoute, changeModel, changeOneM } = claudeRouteState(props);
  if (draft.routeMode === "official") return <p className="asb-field-help">{t("modelRoutes.official")}</p>;
  const native = !!draft.connection?.claudeNative;
  const mode = route?.mode === "provider" ? `provider:${route.profileId}` : route?.mode ?? "automatic";
  return <>
    {role !== "main" && <label className="asb-field"><span>{t("modelRoutes.targetProvider")}</span>
      <Select value={mode} ariaLabel={t("modelRoutes.targetProvider")} disabled={disabled} options={[
        { value: "automatic", label: t(role === "fable" && options.routes.opus ? "modelRoutes.inheritOpus" : "modelRoutes.purposeDefault") },
        { value: "primary", label: t("modelRoutes.primaryModelMode") },
        ...(!native && !claudeTargetIssue(draft) ? [{ value: `provider:${document.profileId}`,
          label: t("providerEditor.selfTarget", { name: draft.name }) }] : []),
        ...(!native ? sources.filter((source) => source.kind === "profile" && source.record.profile.id !== document.profileId
          && !claudeTargetIssue(source.record.profile)).map(({ record }) => ({ value: `provider:${record.profile.id}`, label: record.profile.name })) : []),
      ]} onChange={(next) => changeRoute(next === "automatic" ? null : next === "primary"
        ? { mode: "primary", model: "", oneM: false }
        : { mode: "provider", profileId: next.slice("provider:".length), model: "", oneM: false })} />
    </label>}
    {(role === "main" || route) && <div className="asb-field"><span>{t("modelRoutes.model")}</span>
      {role === "main" ? <p className="asb-field-help">{value || t("modelRoutes.clientDefault")}</p> : <div className="asb-input-with-picker">
        <Input code value={value} readOnly={route?.mode === "provider"} aria-label={t("modelRoutes.model")}
          disabled={disabled} placeholder={t("modelRoutes.clientDefault")} onChange={(event) => changeModel(event.target.value)} />
        {models.length > 0 && <ModelPicker models={models.map((model) => ({ value: model, label: model, group: target?.name ?? "" }))}
          current={value || null} ariaLabel={t("modelRoutes.pickSavedModel")} disabled={disabled} onSelect={changeModel} />}
      </div>}
      <div className="asb-provider-switch-row">
        <span>{t("modelRoutes.oneM")}</span>
        <Select ariaLabel={t("modelRoutes.oneM")} value={String(role === "main" ? options.primaryOneM : route?.oneM ?? false)}
          disabled={disabled || !value.trim()} options={[
            { value: "false", label: t("providerSetup.disabled") }, { value: "true", label: t("providerSetup.enabled") },
          ]} onChange={(next) => changeOneM(next === "true")} />
      </div>
    </div>}
    {role !== "main" && <p className="asb-field-help">{t(route ? "modelRoutes.subagentHelp" : "modelRoutes.followHelp")}</p>}
  </>;
}
