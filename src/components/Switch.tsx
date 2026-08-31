interface Props {
  checked: boolean;
  /** Emits the next state; consumers own what it means for their data. */
  onChange: (checked: boolean) => void;
  /** Accessible name of the control; the row owns the visible label text. */
  label: string;
  disabled?: boolean;
}

/**
 * On/off switch for application-settings rows: the setting text sits in the
 * row's copy column on the left and this control renders on the right, so
 * the layout reads like a conventional settings toggle. A visually hidden
 * native input (upgraded to `role="switch"`) keeps keyboard and screen-reader
 * behavior while the track is drawn beside it. The checked track carries the
 * ported spiralcoder primary-button gradient; all visual values come from
 * styles/tokens.css and the .asb-switch-* rules in base.css.
 */
export function Switch({ checked, disabled = false, label, onChange }: Props) {
  return (
    <label className="asb-switch" data-disabled={disabled ? "true" : undefined}>
      <input
        className="asb-switch-input"
        type="checkbox"
        role="switch"
        aria-label={label}
        checked={checked}
        disabled={disabled}
        aria-checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="asb-switch-track" data-checked={checked ? "true" : "false"} aria-hidden="true">
        <span className="asb-switch-thumb" />
      </span>
    </label>
  );
}
