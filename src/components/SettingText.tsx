import type { SettingSpec, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import { Input } from "./Input";
import { RadioOption } from "./RadioOption";
import { Select } from "./Select";
import { Textarea } from "./Textarea";

interface Props {
  spec: SettingSpec;
  value: SettingValue;
  busy: boolean;
  onChange: (value: SettingValue) => void;
}

export function SettingText({ spec, value, busy, onChange }: Props) {
  const { t } = useI18n();
  const label = catalogText(spec.label, t);
  const raw = value.mode === "automatic" ? "" : String(value.value);
  const update = (text: string) => onChange({ mode: "explicit", value: text });
  if (spec.control === "editableChoice") {
    const known = spec.options.some((option) => option.value === raw);
    return <div className="asb-setting-input">
      <Select value={value.mode === "automatic" ? "automatic" : known ? raw : "custom"} disabled={busy} ariaLabel={label}
        options={[{ value: "automatic", label: t("clientConfig.settings.automatic") },
          ...spec.options.map((option) => ({ ...option, label: catalogText(option.label, t) })),
          { value: "custom", label: t("clientConfig.settings.customValue") }]}
        onChange={(next) => next === "automatic" ? onChange({ mode: "automatic" }) : update(next === "custom" ? "" : next)} />
      {value.mode === "explicit" && !known && <Input value={raw} disabled={busy} aria-label={label}
        aria-invalid={!raw.trim()} onChange={(event) => update(event.target.value)} />}
    </div>;
  }
  return <div className="asb-setting-input">
    <div className="asb-segments" role="radiogroup" aria-label={label}>
      <RadioOption name={`${spec.key}-setting`} checked={value.mode === "automatic"} disabled={busy}
        label={t("clientConfig.settings.automatic")} onChange={() => onChange({ mode: "automatic" })} />
      <RadioOption name={`${spec.key}-setting`} checked={value.mode === "explicit"} disabled={busy}
        label={t("clientConfig.settings.customValue")} onChange={() => update("")} />
    </div>
    {value.mode === "explicit" && <Textarea value={raw} disabled={busy} aria-label={label}
      onChange={(event) => update(event.target.value)} />}
  </div>;
}
