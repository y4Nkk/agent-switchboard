import { useState } from "react";
import type { ClaudeModelPreferencesCatalog } from "../../api/settings";
import { useI18n } from "../../i18n";
import { catalogText } from "../../i18n/errors";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { modeledPreferences, object, withModelPreference, type Fragment, type ModelPreference } from "./claude-model-preferences";

interface Props {
  fragment: Fragment;
  catalog: ClaudeModelPreferencesCatalog;
  busy: boolean;
  onChange: (fragment: Fragment) => void;
}

interface RowProps {
  model: string;
  preferences: Fragment;
  catalog: ClaudeModelPreferencesCatalog;
  busy: boolean;
  onChange: (key: ModelPreference, value: string | number | undefined) => void;
}

function ModelPreferencesRow({ model, preferences, catalog, busy, onChange }: RowProps) {
  const { t } = useI18n();
  const inherited = { value: "inherit", label: t("clientConfig.models.inherit") };
  const window = preferences.autoCompactWindow;
  return <div className="asb-model-preferences-fields">
    {(["effortLevel", "maxEffortLevel"] as const).map((key) => <div className="asb-setting-input-row" key={key}>
      <span>{t(key === "effortLevel" ? "clientConfig.models.effort" : "clientConfig.models.cap")}</span>
      <Select disabled={busy} value={typeof preferences[key] === "string" ? String(preferences[key]) : "inherit"}
        ariaLabel={t("clientConfig.models.field", { model, field: t(key === "effortLevel" ? "clientConfig.models.effort" : "clientConfig.models.cap") })}
        options={[inherited, ...(key === "effortLevel" ? catalog.effortOptions : catalog.capOptions)
          .map((option) => ({ value: option.value, label: catalogText(option.label, t) }))]}
        onChange={(value) => onChange(key, value === "inherit" ? undefined : value)} />
    </div>)}
    <div className="asb-setting-input-row">
      <span>{t("clientConfig.models.window")}</span>
      <Select disabled={busy} value={window === "auto" ? "auto" : typeof window === "number" ? "custom" : "inherit"}
        ariaLabel={t("clientConfig.models.field", { model, field: t("clientConfig.models.window") })}
        options={[inherited, { value: "auto", label: t("clientConfig.models.nativeAuto") },
          { value: "custom", label: t("clientConfig.models.customWindow") }]}
        onChange={(value) => onChange("autoCompactWindow", value === "inherit" ? undefined : value === "auto" ? "auto" : catalog.autoCompactMin)} />
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
  const entries = Object.entries(object(fragment.modelSettings))
    .filter(([, value]) => modeledPreferences.some((key) => key in object(value)));
  const validModel = model.length > 0 && model.trim() === model && !/[\u0000-\u001f\u007f]/.test(model);
  const window = pending.autoCompactWindow;
  const validWindow = window === undefined || window === "auto" || (typeof window === "number"
    && Number.isInteger(window) && window >= catalog.autoCompactMin && window <= catalog.autoCompactMax);
  return <section className="asb-toggle-group" aria-label={t("clientConfig.models.title")}>
    <div className="asb-toggle-group-head"><h3 className="asb-section-title">{t("clientConfig.models.title")}</h3></div>
    <p className="asb-field-help">{t("clientConfig.models.help", { min: catalog.autoCompactMin, max: catalog.autoCompactMax })}</p>
    {entries.map(([name, preferences]) => <div className="asb-model-preferences-row" key={name}>
      <code className="asb-model-effort-name">{name}</code>
      <ModelPreferencesRow model={name} preferences={object(preferences)} catalog={catalog} busy={busy}
        onChange={(key, value) => onChange(withModelPreference(fragment, name, key, value))} />
    </div>)}
    <div className="asb-model-preferences-row">
      <Input code value={model} disabled={busy} aria-label={t("clientConfig.models.model")}
        placeholder={t("clientConfig.models.model")} onChange={(event) => setModel(event.target.value)} />
      <ModelPreferencesRow model={model || t("clientConfig.models.newModel")} preferences={pending} catalog={catalog} busy={busy}
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
  </section>;
}
