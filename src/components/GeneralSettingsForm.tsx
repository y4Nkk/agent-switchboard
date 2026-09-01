import type {
  CommonSettingSpec,
  CommonValue,
} from "../api/client";
import { Slider } from "./Slider";
import { Switch } from "./Switch";

interface Props {
  specs: CommonSettingSpec[];
  /** Section order comes from the ownership catalog. */
  groups: string[];
  values: Record<string, CommonValue>;
  busy: boolean;
  onChange: (key: string, value: CommonValue) => void;
  /** Restores one group's directory defaults; `null` restores every group. */
  onResetGroup: (group: string | null) => void;
}

function choiceIndex(spec: CommonSettingSpec, value: CommonValue): number {
  const index =
    typeof value === "string"
      ? spec.options.findIndex((option) => option.value === value)
      : -1;
  return Math.max(index, 0);
}

function RadioOption({
  name,
  checked,
  disabled,
  label,
  onChange,
}: {
  name: string;
  checked: boolean;
  disabled: boolean;
  label: string;
  onChange: () => void;
}) {
  return (
    <label className={`asb-seg-opt${checked ? " is-active" : ""}`}>
      <input
        type="radio"
        name={name}
        checked={checked}
        disabled={disabled}
        onChange={onChange}
      />
      {label}
    </label>
  );
}

function ChoiceControl({
  spec,
  value,
  busy,
  onChange,
}: {
  spec: Exclude<CommonSettingSpec, { control: "toggle" }>;
  value: CommonValue;
  busy: boolean;
  onChange: (value: CommonValue) => void;
}) {
  const name = `${spec.key}-setting`;
  if (spec.control === "slider") {
    return (
      <Slider
        value={choiceIndex(spec, value)}
        min={0}
        max={spec.options.length - 1}
        step={1}
        ariaLabel={spec.label}
        ariaValueText={`${spec.label} ${
          spec.options[choiceIndex(spec, value)]?.label ?? ""
        }`}
        disabled={busy}
        onValueChange={(index) => onChange(spec.options[index]?.value)}
      />
    );
  }

  return (
    <div className="asb-segments" role="radiogroup" aria-label={spec.label}>
      {spec.options.map((option) => (
        <RadioOption
          key={option.value}
          name={name}
          checked={value === option.value}
          disabled={busy}
          label={option.label}
          onChange={() => onChange(option.value)}
        />
      ))}
    </div>
  );
}

/**
 * Plain general-parameter controls: every parameter always carries a
 * concrete value. There is no raw TOML/JSON editor, no client-file preview,
 * and no patch semantics here — every edit changes only desired application
 * state, and each group can be restored to its directory defaults. The page
 * owns the one all-settings restore action alongside saving.
 */
export function GeneralSettingsForm({
  specs,
  groups,
  values,
  busy,
  onChange,
  onResetGroup,
}: Props) {
  return (
    <div className="asb-toggle-list">
      {groups.map((group) => {
        const groupSpecs = specs.filter((spec) => spec.group === group);
        if (groupSpecs.length === 0) return null;
        return (
          <section className="asb-toggle-group" key={group}>
            <div className="asb-toggle-group-head">
              <h3 className="asb-toggle-group-title">{group}</h3>
              <button
                type="button"
                className="asb-btn-secondary"
                disabled={busy}
                onClick={() => onResetGroup(group)}
              >
                恢复默认值
              </button>
            </div>
            {groupSpecs.map((spec) => {
              const value = values[spec.key];
              return (
                <div className="asb-toggle-row asb-choice-row" key={spec.key}>
                  <div className="asb-choice-head">
                    <span className="asb-checkbox-label">{spec.label}</span>
                  </div>
                  {spec.control === "toggle" ? (
                    <Switch
                      label={spec.label}
                      checked={value === true}
                      disabled={busy}
                      onChange={(checked) => onChange(spec.key, checked)}
                    />
                  ) : (
                    <ChoiceControl
                      spec={spec}
                      value={value}
                      busy={busy}
                      onChange={(next) => onChange(spec.key, next)}
                    />
                  )}
                </div>
              );
            })}
          </section>
        );
      })}
    </div>
  );
}
