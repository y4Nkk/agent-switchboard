interface Props {
  checked: boolean;
  /** Emits the next state; consumers own what it means for their data. */
  onChange: (checked: boolean) => void;
  /** Visible label text; also serves as the accessible name of the control. */
  label: string;
  /** Full accessible name for compact visible labels (e.g. an inline "1M"). */
  ariaLabel?: string;
  disabled?: boolean;
  /** 部分选中：控件显示横条，读屏语义为 mixed；点击仍走 onChange。 */
  indeterminate?: boolean;
}

/**
 * Tick-box ported from the spiralcoder reference: a visually hidden native
 * input keeps keyboard and screen-reader behavior while the box is drawn
 * beside it. All visual values come from styles/tokens.css.
 */
export function Checkbox({ checked, disabled = false, indeterminate = false, label, ariaLabel, onChange }: Props) {
  const active = checked || indeterminate;
  return (
    <label className="asb-checkbox" data-disabled={disabled ? "true" : undefined}>
      <input
        className="asb-checkbox-input"
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-checked={indeterminate ? "mixed" : checked}
        aria-label={ariaLabel}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="asb-checkbox-box" data-checked={active ? "true" : "false"} aria-hidden="true">
        {checked || indeterminate ? (
          <svg viewBox="0 0 12 12" fill="none">
            {indeterminate ? (
              <line x1="2.4" y1="6" x2="9.6" y2="6" strokeWidth="1.8" strokeLinecap="round" />
            ) : (
              <polyline
                points="2.4 6.4 5 9 9.6 3.4"
                strokeWidth="1.8"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            )}
          </svg>
        ) : null}
      </span>
      <span className="asb-checkbox-label">{label}</span>
    </label>
  );
}
