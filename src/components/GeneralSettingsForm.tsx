import type {
  CommonSettingSpec,
  CommonValue,
} from "../api/client";
import { Button } from "./Button";
import { RadioOption } from "./RadioOption";
import { Slider } from "./Slider";

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

const automatic: CommonValue = { mode: "automatic" };

function explicit(value: boolean | string): CommonValue {
  return { mode: "explicit", value };
}

function choiceIndex(spec: CommonSettingSpec, value: CommonValue): number {
  if (value.mode === "automatic") return 0;
  const index = spec.options.findIndex((option) => option.value === value.value);
  return index < 0 ? 0 : index + 1;
}

function choiceLabel(spec: CommonSettingSpec, value: CommonValue): string {
  if (value.mode === "automatic") return "自动";
  return spec.options.find((option) => option.value === value.value)?.label ?? "自动";
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
      <div className="asb-slider-control">
        <span className="asb-choice-current" aria-live="polite">
          当前推理：{choiceLabel(spec, value)}
        </span>
        <Slider
          value={choiceIndex(spec, value)}
          min={0}
          max={spec.options.length}
          step={1}
          ariaLabel={spec.label}
          ariaValueText={`${spec.label} ${choiceLabel(spec, value)}`}
          disabled={busy}
          onValueChange={(index) =>
            onChange(index === 0 ? automatic : explicit(spec.options[index - 1].value))
          }
        />
      </div>
    );
  }

  return (
    <div className="asb-segments" role="radiogroup" aria-label={spec.label}>
      <RadioOption
        name={name}
        checked={value.mode === "automatic"}
        disabled={busy}
        label="自动"
        onChange={() => onChange(automatic)}
      />
      {spec.options.map((option) => (
        <RadioOption
          key={option.value}
          name={name}
          checked={value.mode === "explicit" && value.value === option.value}
          disabled={busy}
          label={option.label}
          onChange={() => onChange(explicit(option.value))}
        />
      ))}
    </div>
  );
}

/**
 * General parameters record either automatic client behavior or one explicit
 * application-owned value. There is no raw TOML/JSON editor: every edit
 * changes only desired application state, and every reset returns a setting
 * to automatic behavior.
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
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() => onResetGroup(group)}
              >
                恢复默认值
              </Button>
            </div>
            {groupSpecs.map((spec) => {
              const value = values[spec.key] ?? automatic;
              return (
                <div className="asb-toggle-row asb-choice-row" key={spec.key}>
                  <div className="asb-choice-head">
                    <span className="asb-checkbox-label">{spec.label}</span>
                  </div>
                  {spec.control === "toggle" ? (
                    <div className="asb-segments" role="radiogroup" aria-label={spec.label}>
                      <RadioOption
                        name={`${spec.key}-setting`}
                        checked={value.mode === "automatic"}
                        disabled={busy}
                        label="自动"
                        onChange={() => onChange(spec.key, automatic)}
                      />
                      <RadioOption
                        name={`${spec.key}-setting`}
                        checked={value.mode === "explicit" && value.value === true}
                        disabled={busy}
                        label="开启"
                        onChange={() => onChange(spec.key, explicit(true))}
                      />
                      <RadioOption
                        name={`${spec.key}-setting`}
                        checked={value.mode === "explicit" && value.value === false}
                        disabled={busy}
                        label="关闭"
                        onChange={() => onChange(spec.key, explicit(false))}
                      />
                    </div>
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
