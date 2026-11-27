import { RadioOption } from "./RadioOption";
import { SettingsRow } from "./SettingsLayout";

export function AppSegmentSetting<T extends string | number>({ label, detail, value, options, busy, onChange }: {
  label: string;
  detail?: string;
  value: T;
  options: readonly { value: T; label: string }[];
  busy: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <SettingsRow label={label} description={detail}>
      <div className="asb-segments" role="radiogroup" aria-label={label}>
        {options.map((option) => (
          <RadioOption key={option.value} name={label} label={option.label}
            checked={value === option.value} disabled={busy} onChange={() => onChange(option.value)} />
        ))}
      </div>
    </SettingsRow>
  );
}
