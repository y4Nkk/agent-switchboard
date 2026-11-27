import * as SelectPrimitive from "@radix-ui/react-select";
import { useContext } from "react";
import { useDialogFieldPopup } from "./AppDialog";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from "./icons";
import { OverlayMotionContext } from "./overlay-motion";
import { Tooltip } from "./Tooltip";

export interface SelectOption {
  value: string;
  label: string;
  tooltip?: string;
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
  contentClassName?: string;
  triggerTooltip?: string;
}

/**
 * Dropdown picker:
 * field-shaped trigger with a trailing chevron, frosted-menu popover with
 * the chosen item marked by a right-aligned check. Rendering contract and
 * visuals are owned here; all values come from styles/tokens.css.
 */
export function Select({ value, options, onChange, placeholder, ariaLabel, disabled = false,
  contentClassName, triggerTooltip }: Props) {
  const animate = useContext(OverlayMotionContext);
  // Radix handles an outside press before React Aria completes the gesture.
  const popup = useDialogFieldPopup(disabled, true);
  // An open tooltip receives Escape before the listbox layer does.
  const closeFromOptionTooltip = (event: KeyboardEvent) => {
    event.stopPropagation();
    popup.onOpenChange(false);
  };
  const trigger = <SelectPrimitive.Trigger className="asb-select-trigger" aria-label={ariaLabel}>
    {/* Radix Value discards className, so its wrapper owns truncation. */}
    <span className="asb-select-value"><SelectPrimitive.Value placeholder={placeholder} /></span>
    <SelectPrimitive.Icon className="asb-select-chevron">
      <ChevronDownIcon />
    </SelectPrimitive.Icon>
  </SelectPrimitive.Trigger>;
  return (
    <SelectPrimitive.Root
      value={value ?? ""}
      onValueChange={onChange}
      disabled={popup.disabled}
      open={popup.open}
      onOpenChange={popup.onOpenChange}
    >
      {triggerTooltip ? <Tooltip label={triggerTooltip} side="bottom" enabled={!popup.open}>{trigger}</Tooltip> : trigger}
      <SelectPrimitive.Portal container={popup.container}>
        <SelectPrimitive.Content
          className={["asb-select-content", contentClassName].filter(Boolean).join(" ")}
          data-overlay-motion={animate || undefined}
          position="popper"
          sideOffset={4}
          onEscapeKeyDown={(event) => event.stopPropagation()}
        >
          <SelectPrimitive.ScrollUpButton className="asb-select-scroll">
            <ChevronUpIcon />
          </SelectPrimitive.ScrollUpButton>
          <SelectPrimitive.Viewport className="asb-select-viewport">
            {options.map(({ value: optionValue, label, tooltip }) => (
              <Tooltip key={optionValue} label={tooltip ?? label} side="right"
                onEscapeKeyDown={closeFromOptionTooltip}>
                <SelectPrimitive.Item asChild value={optionValue} aria-label={tooltip ?? label}>
                  <span className="asb-select-item">
                    <SelectPrimitive.ItemText>{label}</SelectPrimitive.ItemText>
                    <span className="asb-select-check" aria-hidden="true">
                      <SelectPrimitive.ItemIndicator>
                        <CheckIcon />
                      </SelectPrimitive.ItemIndicator>
                    </span>
                  </span>
                </SelectPrimitive.Item>
              </Tooltip>
            ))}
          </SelectPrimitive.Viewport>
          <SelectPrimitive.ScrollDownButton className="asb-select-scroll">
            <ChevronDownIcon />
          </SelectPrimitive.ScrollDownButton>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
