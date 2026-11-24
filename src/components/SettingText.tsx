import { useId, useState } from "react";
import type { SettingSpec, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import { Input } from "./Input";
import { RadioOption } from "./RadioOption";
import { Textarea } from "./Textarea";

interface Props {
  spec: SettingSpec;
  value: SettingValue;
  busy: boolean;
  onChange: (value: SettingValue) => void;
}

export function SettingText({ spec, value, busy, onChange }: Props) {
  const { t } = useI18n();
  const name = useId();
  const [custom, setCustom] = useState(false);
  const label = catalogText(spec.label, t);
  const raw = value.mode === "automatic" ? "" : String(value.value);
  const update = (text: string) => onChange({ mode: "explicit", value: text });
  if (spec.control === "editableChoice") {
    const known = spec.options.some((option) => option.value === raw);
    const editing = value.mode === "explicit" && (custom || !known);
    return <div className="asb-setting-input">
      <div className="asb-segments" role="radiogroup" aria-label={label}>
        <RadioOption name={name} checked={value.mode === "automatic"} disabled={busy}
          label={t("clientConfig.settings.automatic")} onChange={() => { setCustom(false); onChange({ mode: "automatic" }); }} />
        {spec.options.map((option) => <RadioOption key={option.value} name={name}
          checked={value.mode === "explicit" && !editing && option.value === raw} disabled={busy}
          label={catalogText(option.label, t)} onChange={() => { setCustom(false); update(option.value); }} />)}
        <RadioOption name={name} checked={editing} disabled={busy}
          label={t("clientConfig.settings.customValue")} onChange={() => { setCustom(true); update(""); }} />
      </div>
      {editing && <Input value={raw} disabled={busy} aria-label={label}
        aria-invalid={!raw.trim()} onChange={(event) => update(event.target.value)} />}
    </div>;
  }
  return <div className="asb-setting-input">
    <div className="asb-segments" role="radiogroup" aria-label={label}>
      <RadioOption name={name} checked={value.mode === "automatic"} disabled={busy}
        label={t("clientConfig.settings.automatic")} onChange={() => onChange({ mode: "automatic" })} />
      <RadioOption name={name} checked={value.mode === "explicit"} disabled={busy}
        label={t("clientConfig.settings.customValue")} onChange={() => update("")} />
    </div>
    {value.mode === "explicit" && <Textarea value={raw} disabled={busy} aria-label={label}
      onChange={(event) => update(event.target.value)} />}
  </div>;
}
