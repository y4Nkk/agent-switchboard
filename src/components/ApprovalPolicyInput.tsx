import type { ConfigValue, SettingSpec, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import { Checkbox } from "./Checkbox";
import { Select } from "./Select";

interface Props {
  spec: SettingSpec;
  value: SettingValue;
  busy: boolean;
  onChange: (value: SettingValue) => void;
}

function object(value: ConfigValue | undefined): Record<string, ConfigValue> | null {
  return value !== undefined && typeof value === "object" && !Array.isArray(value) ? value : null;
}

export function ApprovalPolicyInput({ spec, value, busy, onChange }: Props) {
  const { t } = useI18n();
  const granular = value.mode === "explicit" ? object(object(value.value)?.granular) : null;
  const mode = value.mode === "automatic" ? "automatic" : granular ? "granular" : String(value.value);
  const update = (fields: Record<string, ConfigValue>) => onChange({ mode: "explicit", value: { granular: fields } });
  return <div className="asb-setting-input">
    <Select value={mode} disabled={busy} ariaLabel={catalogText(spec.label, t)} options={[
      { value: "automatic", label: t("clientConfig.settings.automatic") },
      { value: "on-request", label: t("ownership.option.approval_policy.on-request") },
      { value: "never", label: t("ownership.option.approval_policy.never") },
      { value: "granular", label: t("clientConfig.settings.granularApproval") },
    ]} onChange={(mode) => {
      if (mode === "automatic") onChange({ mode: "automatic" });
      else if (mode === "granular") update(Object.fromEntries(spec.options.filter((field) => field.required).map((field) => [field.value, true])));
      else onChange({ mode: "explicit", value: mode });
    }} />
    {granular && <>
      <p className="asb-field-help">{t("clientConfig.settings.granularHelp")}</p>
      {spec.options.map((field) => field.required
        ? <Checkbox key={field.value} label={catalogText(field.label, t)} disabled={busy}
            checked={granular[field.value] === true} onChange={(enabled) => update({ ...granular, [field.value]: enabled })} />
        : <div className="asb-setting-input-row" key={field.value}>
            <span>{catalogText(field.label, t)}</span>
            <Select value={field.value in granular ? String(granular[field.value]) : "automatic"} disabled={busy}
              ariaLabel={catalogText(field.label, t)} options={[
                { value: "automatic", label: t("clientConfig.settings.nativeOff") },
                { value: "true", label: t("clientConfig.settings.allowPrompt") },
                { value: "false", label: t("clientConfig.settings.rejectPrompt") },
              ]} onChange={(next) => {
                const fields = { ...granular };
                if (next === "automatic") delete fields[field.value];
                else fields[field.value] = next === "true";
                update(fields);
              }} />
          </div>)}
    </>}
  </div>;
}
