import type { SettingSpec, SettingValue } from "../../api/client";
import { catalogText } from "../../i18n/errors";
import { useI18n } from "../../i18n";
import { Input } from "../Input";
import { Select } from "../Select";

/** One always-on control per parameter: closed choices are a Select with the
 * client default as an option, numbers are a field whose empty value IS the
 * default — no edit round-trip, no mode picker. */
export function ProviderParameterControl({ spec, value, busy, onChange }: {
  spec: SettingSpec; value: SettingValue; busy: boolean; onChange: (value: SettingValue) => void;
}) {
  const { t } = useI18n();
  const automatic = { value: "automatic", label: t("providerSetup.defaults") };
  const choices = spec.control === "toggle"
    ? [{ value: "true", label: t("providerSetup.enabled") }, { value: "false", label: t("providerSetup.disabled") }]
    : spec.control === "slider" || spec.control === "segment"
      ? spec.options.map((option) => ({ value: option.value, label: catalogText(option.label, t) })) : null;
  const numeric = spec.control === "integer" || spec.control === "stringInteger";
  return <div className="asb-provider-parameter-value">
    {choices ? <Select ariaLabel={catalogText(spec.label, t)} disabled={busy}
        value={value.mode === "automatic" ? "automatic" : String(value.value)}
        options={[automatic, ...choices]}
        onChange={(next) => onChange(next === "automatic" ? { mode: "automatic" }
          : { mode: "explicit", value: spec.control === "toggle" ? next === "true" : next })} />
      : <Input type="number" step={1} disabled={busy} placeholder={t("providerSetup.defaults")}
        aria-label={catalogText(spec.label, t)}
        min={"min" in spec ? spec.min : undefined} max={"max" in spec ? spec.max : undefined}
        value={value.mode === "automatic" ? "" : String(value.value)}
        onChange={(event) => { const raw = event.target.value;
          onChange(raw === "" ? { mode: "automatic" }
            : { mode: "explicit", value: spec.control === "integer" ? Number(raw) : raw }); }} />}
    {value.mode !== "automatic" && <span className="asb-field-help">{t("providerSetup.custom")}</span>}
  </div>;
}
