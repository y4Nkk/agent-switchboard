import * as SelectPrimitive from "@radix-ui/react-select";

export interface SelectOption {
  value: string;
  label: string;
}

interface Props {
  /** Currently chosen option value; null = nothing chosen yet. */
  value: string | null;
  options: readonly SelectOption[];
  /** Emits the newly chosen option value. */
  onChange: (value: string) => void;
  /** Trigger text while no value is set. */
  placeholder?: string;
  /** Accessible name of the combobox trigger. */
  ariaLabel: string;
  disabled?: boolean;
}

function ChevronIcon({ up = false }: { up?: boolean }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d={up ? "M3.5 10 L8 5.5 L12.5 10" : "M3.5 6 L8 10.5 L12.5 6"}
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <polyline
        points="2.4 6.4 5 9 9.6 3.4"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * Dropdown picker ported from the spiralcoder reference (Radix Select):
 * field-shaped trigger with a trailing chevron, frosted-menu popover with
 * the chosen item marked by a right-aligned check. Rendering contract and
 * visuals are owned here; all values come from styles/tokens.css.
 */
export function Select({
  value,
  options,
  onChange,
  placeholder,
  ariaLabel,
  disabled = false,
}: Props) {
  return (
    <SelectPrimitive.Root value={value ?? ""} onValueChange={onChange} disabled={disabled}>
      <SelectPrimitive.Trigger className="asb-select-trigger" aria-label={ariaLabel}>
        <SelectPrimitive.Value placeholder={placeholder} />
        <SelectPrimitive.Icon className="asb-select-chevron">
          <ChevronIcon />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Content className="asb-select-content" position="popper" sideOffset={4}>
          <SelectPrimitive.ScrollUpButton className="asb-select-scroll">
            <ChevronIcon up />
          </SelectPrimitive.ScrollUpButton>
          <SelectPrimitive.Viewport className="asb-select-viewport">
            {options.map(({ value: optionValue, label }) => (
              <SelectPrimitive.Item key={optionValue} value={optionValue} className="asb-select-item">
                <SelectPrimitive.ItemText>{label}</SelectPrimitive.ItemText>
                <span className="asb-select-check" aria-hidden="true">
                  <SelectPrimitive.ItemIndicator>
                    <CheckIcon />
                  </SelectPrimitive.ItemIndicator>
                </span>
              </SelectPrimitive.Item>
            ))}
          </SelectPrimitive.Viewport>
          <SelectPrimitive.ScrollDownButton className="asb-select-scroll">
            <ChevronIcon />
          </SelectPrimitive.ScrollDownButton>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
