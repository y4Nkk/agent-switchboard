import { useId, useState } from "react";
import type { ClaudeModelPreferencesCatalog } from "../../api/settings";
import { useI18n } from "../../i18n";
import { catalogText } from "../../i18n/errors";
import { Button } from "../Button";
import { Input } from "../Input";
import { SettingsGroup } from "../SettingsLayout";
import { RadioOption } from "../RadioOption";
import { modeledPreferences, object, withModelPreference, type Fragment, type ModelPreference } from "./claude-model-preferences";

interface Props {
  fragment: Fragment;
  catalog: ClaudeModelPreferencesCatalog;
  busy: boolean;
  onChange: (fragment: Fragment) => void;
}

interface RowProps {
  /** Radio-group scope: unique per row so equal field names never merge groups. */
  scope: string;
  /** Model named in the accessible labels. */
  model: string;
  preferences: Fragment;
  catalog: ClaudeModelPreferencesCatalog;
  busy: boolean;
  onChange: (key: ModelPreference, value: string | number | undefined) => void;
}

function ModelPreferencesRow({ scope, model, preferences, catalog, busy, onChange }: RowProps) {
  const { t } = useI18n();
  const window = preferences.autoCompactWindow;
  const rail = (field: ModelPreference, label: string, current: string | null,
    options: ReadonlyArray<{ value: string; label: string }>, apply: (value: string) => void) =>
    <div className="asb-segments" role="radiogroup" aria-label={t("clientConfig.models.field", { model, field: label })}>
      {[{ value: "inherit", label: t("clientConfig.models.inherit") }, ...options].map((option) => (
        <RadioOption key={option.value} name={`${scope}-${field}`} checked={current === option.value} disabled={busy}
          label={option.label} onChange={() => apply(option.value)} />))}
    </div>;
  return <div className="asb-model-preferences-fields">
    {(["effortLevel", "maxEffortLevel"] as const).map((key) => {
      const label = t(key === "effortLevel" ? "clientConfig.models.effort" : "clientConfig.models.cap");
      return <div className="asb-setting-input-row" key={key}>
        <span>{label}</span>
        {rail(key, label, typeof preferences[key] === "string" ? String(preferences[key]) : "inherit",
          (key === "effortLevel" ? catalog.effortOptions : catalog.capOptions)
            .map((option) => ({ value: option.value, label: catalogText(option.label, t) })),
          (value) => onChange(key, value === "inherit" ? undefined : value))}
      </div>;
    })}
    <div className="asb-setting-input-row">
      <span>{t("clientConfig.models.window")}</span>
      {rail("autoCompactWindow", t("clientConfig.models.window"),
        window === "auto" ? "auto" : typeof window === "number" ? "custom" : "inherit",
        [{ value: "auto", label: t("clientConfig.models.nativeAuto") },
          { value: "custom", label: t("clientConfig.models.customWindow") }],
        (value) => onChange("autoCompactWindow",
          value === "inherit" ? undefined : value === "auto" ? "auto" : catalog.autoCompactMin))}
      {typeof window === "number" && <Input code type="number" value={window} min={catalog.autoCompactMin} max={catalog.autoCompactMax}
        step={1} disabled={busy} aria-label={t("clientConfig.models.windowTokens", { model })}
        aria-invalid={!Number.isInteger(window) || window < catalog.autoCompactMin || window > catalog.autoCompactMax}
        onChange={(event) => onChange("autoCompactWindow", Number(event.target.value))} />}
    </div>
  </div>;
}

export function ClaudeModelPreferencesEditor({ fragment, catalog, busy, onChange }: Props) {
  const { t } = useI18n();
  const [model, setModel] = useState("");
  const [pending, setPending] = useState<Fragment>({});
  const newScope = useId();
  const entries = Object.entries(object(fragment.modelSettings))
    .filter(([, value]) => modeledPreferences.some((key) => key in object(value)));
  const validModel = model.length > 0 && model.trim() === model && !/[\u0000-\u001f\u007f]/.test(model);
  const window = pending.autoCompactWindow;
  const validWindow = window === undefined || window === "auto" || (typeof window === "number"
    && Number.isInteger(window) && window >= catalog.autoCompactMin && window <= catalog.autoCompactMax);
  return <SettingsGroup title={t("clientConfig.models.title")}
    description={t("clientConfig.models.help", { min: catalog.autoCompactMin, max: catalog.autoCompactMax })}>
    {entries.map(([name, preferences]) => <div className="asb-model-preferences-row" key={name}>
      <code className="asb-model-effort-name">{name}</code>
      <ModelPreferencesRow scope={`${newScope}-${name}`} model={name} preferences={object(preferences)} catalog={catalog} busy={busy}
        onChange={(key, value) => onChange(withModelPreference(fragment, name, key, value))} />
    </div>)}
    <div className="asb-model-preferences-row">
      <Input code value={model} disabled={busy} aria-label={t("clientConfig.models.model")}
        placeholder={t("clientConfig.models.model")} onChange={(event) => setModel(event.target.value)} />
      <ModelPreferencesRow scope={newScope} model={model || t("clientConfig.models.newModel")} preferences={pending}
        catalog={catalog} busy={busy}
        onChange={(key, value) => setPending((previous) => {
          const next = { ...previous };
          if (value === undefined) delete next[key]; else next[key] = value;
          return next;
        })} />
      <Button variant="secondary" disabled={busy || !validModel || !validWindow || !Object.keys(pending).length} onClick={() => {
        const next = modeledPreferences.reduce((draft, key) => key in pending
          ? withModelPreference(draft, model, key, pending[key] as string | number) : draft, fragment);
        onChange(next); setModel(""); setPending({});
      }}>{t("clientConfig.models.add")}</Button>
    </div>
  </SettingsGroup>;
}
