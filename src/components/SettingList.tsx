import type { SettingSpec, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import { Button } from "./Button";
import { Input } from "./Input";
import { RadioOption } from "./RadioOption";

interface Props {
  spec: SettingSpec;
  value: SettingValue;
  busy: boolean;
  onChange: (value: SettingValue) => void;
}

export function SettingList({ spec, value, busy, onChange }: Props) {
  const { t } = useI18n();
  const items = value.mode === "explicit" && Array.isArray(value.value)
    && value.value.every((item) => typeof item === "string") ? value.value as string[] : null;
  const update = (next: string[]) => onChange({ mode: "explicit", value: next });
  return <div className="asb-setting-input">
    <div className="asb-segments" role="radiogroup" aria-label={catalogText(spec.label, t)}>
      <RadioOption name={`${spec.key}-setting`} checked={value.mode === "automatic"} disabled={busy}
        label={t("clientConfig.settings.automatic")} onChange={() => onChange({ mode: "automatic" })} />
      {spec.control === "notifications" && [true, false].map((enabled) => <RadioOption key={String(enabled)}
        name={`${spec.key}-setting`} checked={value.mode === "explicit" && value.value === enabled} disabled={busy}
        label={t(enabled ? "clientConfig.settings.on" : "clientConfig.settings.off")}
        onChange={() => onChange({ mode: "explicit", value: enabled })} />)}
      <RadioOption name={`${spec.key}-setting`} checked={items !== null} disabled={busy}
        label={t("clientConfig.settings.customList")} onChange={() => update([])} />
    </div>
    {items !== null && <div className="asb-setting-input">
      {items.map((item, index) => <div className="asb-setting-input-row" key={index}>
        <Input code value={item} disabled={busy}
          aria-label={t("clientConfig.settings.listItem", { label: catalogText(spec.label, t), index: index + 1 })}
          aria-invalid={!item.trim()} onChange={(event) => update(items.map((old, i) => i === index ? event.target.value : old))} />
        <Button variant="secondary" disabled={busy} aria-label={t("clientConfig.settings.removeItem", { index: index + 1 })}
          onClick={() => update(items.filter((_, i) => i !== index))}>{t("clientConfig.settings.remove")}</Button>
      </div>)}
      {items.length === 0 && <span className="asb-field-help">{t("clientConfig.settings.emptyList")}</span>}
      <Button variant="secondary" disabled={busy} onClick={() => update([...items, ""])}>{t("clientConfig.settings.addItem")}</Button>
    </div>}
  </div>;
}
