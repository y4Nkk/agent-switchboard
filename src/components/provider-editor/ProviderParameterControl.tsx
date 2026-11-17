import type { SettingSpec, SettingValue } from "../../api/client";
import { catalogText } from "../../i18n/errors";
import { useI18n } from "../../i18n";
import { ProviderValue } from "./ProviderValue";

export function ProviderParameterControl({ spec, value, display, busy, onChange }: {
  spec: SettingSpec; value: SettingValue; display: string; busy: boolean; onChange: (value: SettingValue) => void;
}) {
  const { t } = useI18n();
  const automatic = { value: "automatic", label: t("providerSetup.defaults") };
  const choices = spec.control === "toggle"
    ? [{ value: "true", label: t("providerSetup.enabled") }, { value: "false", label: t("providerSetup.disabled") }]
    : spec.control === "slider" || spec.control === "segment"
      ? spec.options.map((option) => ({ value: option.value, label: catalogText(option.label, t) })) : null;
  const numeric = spec.control === "integer" || spec.control === "stringInteger";
  const raw = value.mode === "automatic" ? choices ? "automatic" : "" : String(value.value);
  return <div className="asb-provider-parameter-value">
    <ProviderValue label={catalogText(spec.label, t)} value={raw} display={value.mode === "automatic"
      ? t("providerSetup.defaults") : display} disabled={busy}
      options={choices ? [automatic, ...choices] : undefined} numeric={numeric}
      min={"min" in spec ? spec.min : undefined} max={"max" in spec ? spec.max : undefined}
      onChange={(next) => onChange(next === "" || choices && next === "automatic" ? { mode: "automatic" }
        : { mode: "explicit", value: spec.control === "integer" ? Number(next) : spec.control === "toggle" ? next === "true" : next })} />
    <span className="asb-field-help">{t(value.mode === "automatic" ? "providerSetup.defaults" : "providerSetup.custom")}</span>
  </div>;
}
