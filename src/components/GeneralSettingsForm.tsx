import type { AppKind } from "../api/client";
import type { ChoiceState, ToggleState } from "../api/client";
import { Checkbox } from "./Checkbox";
import { Slider } from "./Slider";
import { Tooltip } from "./Tooltip";

interface Props {
  app: AppKind;
  toggles: ToggleState[];
  choices: ChoiceState[];
  /** Section order; every catalog entry must name a group from this list. */
  groups: string[];
  busy: boolean;
  /** Checked = the applied line lands in the config file; unchecked = the
      line is removed. Applied immediately through the safe write path. */
  onToggle: (toggle: ToggleState, checked: boolean) => void;
  /** Selecting an option writes that line; null = 默认 (line removed). */
  onChoiceChange: (choice: ChoiceState, value: string | null) => void;
}

const DEFAULT_LABEL = "默认";

function choiceValueLabel(choice: ChoiceState): string {
  if (choice.value === null) return DEFAULT_LABEL;
  const option = choice.options.find(({ value }) => value === choice.value);
  return option ? option.label : `${DEFAULT_LABEL}（${choice.value}）`;
}

function choiceLinePreview(app: AppKind, choice: ChoiceState): string {
  if (choice.value === null) {
    return `默认：不写入 ${choice.key}，由客户端内置默认值决定`;
  }
  return app === "codex"
    ? `${choice.key} = "${choice.value}"`
    : `"${choice.key}": "${choice.value}"`;
}

function optionIndex(choice: ChoiceState): number {
  const index = choice.options.findIndex(({ value }) => value === choice.value);
  return index < 0 ? 0 : index + 1;
}

function optionValue(choice: ChoiceState, index: number): string | null {
  return index === 0 ? null : choice.options[index - 1].value;
}

/**
 * General-configuration controls over official client settings, grouped into
 * sections. Checkboxes, segments, and the reasoning-effort slider each manage
 * one real config line; changes land in the client's config file through the
 * safe write path. They edit a typed patch; they never produce configuration
 * text. Model routing belongs to provider profiles.
 */
export function GeneralSettingsForm({
  app,
  toggles,
  choices,
  groups,
  busy,
  onToggle,
  onChoiceChange,
}: Props) {
  return (
    <div className="asb-toggle-list">
      {groups.map((group) => {
        const groupToggles = toggles.filter((toggle) => toggle.group === group);
        const groupChoices = choices.filter((choice) => choice.group === group);
        if (groupToggles.length === 0 && groupChoices.length === 0) return null;
        return (
          <section className="asb-toggle-group" key={group}>
            <h3 className="asb-toggle-group-title">{group}</h3>
            {groupToggles.map((toggle) => (
              <div className="asb-toggle-row" key={toggle.key}>
                <Checkbox
                  label={toggle.label}
                  checked={toggle.value}
                  disabled={busy}
                  onChange={(checked) => onToggle(toggle, checked)}
                />
                <code className="asb-toggle-line">{toggle.line}</code>
              </div>
            ))}
            {groupChoices.map((choice) => {
              const currentLine =
                choice.value === null ? null : (
                  <code className="asb-toggle-line">{choiceLinePreview(app, choice)}</code>
                );
              return choice.control === "slider" ? (
                <div className="asb-toggle-row asb-choice-row" key={choice.key}>
                  <div className="asb-choice-head">
                    <span className="asb-checkbox-label">{choice.label}</span>
                    <Tooltip side="left" label={choiceLinePreview(app, choice)}>
                      <span
                        className="asb-choice-value"
                        tabIndex={0}
                        aria-label={`当前${choice.label} ${choiceValueLabel(choice)}`}
                      >
                        {choiceValueLabel(choice)}
                      </span>
                    </Tooltip>
                  </div>
                  <Slider
                    value={optionIndex(choice)}
                    min={0}
                    max={choice.options.length}
                    step={1}
                    ariaLabel={choice.label}
                    ariaValueText={`${choice.label} ${choiceValueLabel(choice)}`}
                    disabled={busy}
                    onValueChange={(index) => onChoiceChange(choice, optionValue(choice, index))}
                  />
                  {currentLine}
                </div>
              ) : (
                <div className="asb-toggle-row asb-choice-row" key={choice.key}>
                  <div className="asb-choice-head">
                    <span className="asb-checkbox-label">{choice.label}</span>
                    <Tooltip side="left" label={choiceLinePreview(app, choice)}>
                      <span
                        className="asb-choice-value"
                        tabIndex={0}
                        aria-label={`当前${choice.label} ${choiceValueLabel(choice)}`}
                      >
                        {choiceValueLabel(choice)}
                      </span>
                    </Tooltip>
                  </div>
                  <div className="asb-segments" role="radiogroup" aria-label={choice.label}>
                    {[null, ...choice.options.map(({ value }) => value)].map((value) => {
                      const active = choice.value === value;
                      const label =
                        value === null
                          ? DEFAULT_LABEL
                          : choice.options.find((option) => option.value === value)!.label;
                      return (
                        <label
                          className={`asb-seg-opt${active ? " is-active" : ""}`}
                          key={value ?? "default"}
                        >
                          <input
                            type="radio"
                            name={`${app}-${choice.key}`}
                            value={value ?? ""}
                            checked={active}
                            disabled={busy}
                            onChange={() => onChoiceChange(choice, value)}
                          />
                          {label}
                        </label>
                      );
                    })}
                  </div>
                  {currentLine}
                </div>
              );
            })}
          </section>
        );
      })}
    </div>
  );
}
