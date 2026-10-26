import { useLayoutEffect, useRef } from "react";
import { CheckIcon, DashIcon } from "./icons";

interface Props {
  checked: boolean;
  /** Emits the next state; consumers own what it means for their data. */
  onChange: (checked: boolean) => void;
  /** Visible label text; also serves as the accessible name of the control. */
  label: string;
  /** Full accessible name for compact visible labels (e.g. an inline "1M"). */
  ariaLabel?: string;
  disabled?: boolean;
  ariaBusy?: boolean;
  /** 部分选中：控件显示横条，读屏语义为 mixed；点击仍走 onChange。 */
  indeterminate?: boolean;
}

/**
 * Tick-box with a visually hidden native
 * input keeps keyboard and screen-reader behavior while the box is drawn
 * beside it. All visual values come from styles/tokens.css.
 */
export function Checkbox({ checked, disabled = false, indeterminate = false, label, ariaLabel, ariaBusy, onChange }: Props) {
  const input = useRef<HTMLInputElement>(null);
  // A native click clears this DOM-only property; restore controlled intent
  // on each commit, including while an asynchronous change is pending.
  useLayoutEffect(() => {
    if (input.current) input.current.indeterminate = indeterminate;
  });
  const active = checked || indeterminate;
  return (
    <label className="asb-checkbox" data-disabled={disabled ? "true" : undefined}>
      <input
        ref={input}
        className="asb-checkbox-input"
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-checked={indeterminate ? "mixed" : checked}
        aria-label={ariaLabel}
        aria-busy={ariaBusy || undefined}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="asb-checkbox-box" data-checked={active ? "true" : "false"} aria-hidden="true">
        {indeterminate ? <DashIcon /> : checked ? <CheckIcon /> : null}
      </span>
      <span className="asb-checkbox-label">{label}</span>
    </label>
  );
}
