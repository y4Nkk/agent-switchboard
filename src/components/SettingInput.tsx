import type { SettingSpec, SettingValue } from "../api/client";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import { ApprovalPolicyInput } from "./ApprovalPolicyInput";
import { Input } from "./Input";
import { SettingList } from "./SettingList";
import { SettingText } from "./SettingText";

interface Props {
  spec: SettingSpec;
  value: SettingValue;
  busy: boolean;
  onChange: (value: SettingValue) => void;
}

export function SettingInput(props: Props) {
  const { spec, value, busy, onChange } = props;
  const { t } = useI18n();
  if (spec.control === "notifications" || spec.control === "stringList") return <SettingList {...props} />;
  if (spec.control === "multilineText" || spec.control === "editableChoice") return <SettingText {...props} />;
  if (spec.control === "approvalPolicy") return <ApprovalPolicyInput {...props} />;
  if (spec.control !== "text" && spec.control !== "integer" && spec.control !== "stringInteger") return null;
  const numeric = spec.control !== "text";
  const bounds = "min" in spec ? spec : undefined;
  const raw = value.mode === "automatic" ? "" : String(value.value);
  return <div className="asb-setting-input">
    <Input code={numeric} aria-label={catalogText(spec.label, t)} disabled={busy} value={raw}
      type={numeric ? "number" : "text"} min={bounds?.min} max={bounds?.max}
      step={numeric ? 1 : undefined} placeholder={t("clientConfig.settings.automatic")}
      onChange={(event) => {
        const text = event.target.value;
        onChange(text === "" ? { mode: "automatic" } : {
          mode: "explicit", value: spec.control === "integer" ? Number(text) : text,
        });
      }} />
    {bounds && <span className="asb-field-help">{t("clientConfig.settings.integerRange", { min: bounds.min, max: bounds.max })}</span>}
  </div>;
}
