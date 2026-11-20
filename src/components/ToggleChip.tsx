interface Props {
  pressed: boolean;
  /** Visible chip text; doubles as the accessible name unless ariaLabel is set. */
  label: string;
  ariaLabel?: string;
  disabled?: boolean;
  /** Emits the next pressed state; consumers own what membership means. */
  onClick: () => void;
}

/**
 * One-click membership toggle: the pressed state IS the value, so scanning
 * and editing read the same pixels — no edit round-trip. Capabilities,
 * reasoning levels and limit presets all project onto this single primitive;
 * geometry lives in the .asb-chip* rules of provider-editor-sections.css.
 */
export function ToggleChip({ pressed, label, ariaLabel, disabled = false, onClick }: Props) {
  return (
    <button type="button" className="asb-chip" aria-pressed={pressed} aria-label={ariaLabel} disabled={disabled} onClick={onClick}>
      {label}
    </button>
  );
}
