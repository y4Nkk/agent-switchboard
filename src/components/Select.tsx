import * as SelectPrimitive from "@radix-ui/react-select";
import { useRef, useState } from "react";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from "./icons";

export interface SelectOption {
  value: string;
  label: string;
  title?: string;
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
}

/**
 * Dropdown picker:
 * field-shaped trigger with a trailing chevron, frosted-menu popover with
 * the chosen item marked by a right-aligned check. Rendering contract and
 * visuals are owned here; all values come from styles/tokens.css.
 */
export function Select({ value, options, onChange, placeholder, ariaLabel, disabled = false, contentClassName }: Props) {
  const trigger = useRef<HTMLButtonElement>(null);
  const [container, setContainer] = useState<HTMLElement>();
  return (
    <SelectPrimitive.Root
      value={value ?? ""}
      onValueChange={onChange}
      disabled={disabled}
      onOpenChange={(open) => {
        // Keep the listbox inside its modal's focus and accessibility boundary.
        if (open) setContainer(trigger.current?.closest<HTMLElement>('[role="dialog"]') ?? undefined);
      }}
    >
      <SelectPrimitive.Trigger ref={trigger} className="asb-select-trigger" aria-label={ariaLabel}>
        {/* Radix Value discards className, so its wrapper owns truncation. */}
        <span className="asb-select-value"><SelectPrimitive.Value placeholder={placeholder} /></span>
        <SelectPrimitive.Icon className="asb-select-chevron">
          <ChevronDownIcon />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal container={container}>
        <SelectPrimitive.Content
          className={["asb-select-content", contentClassName].filter(Boolean).join(" ")}
          position="popper"
          sideOffset={4}
          onEscapeKeyDown={(event) => event.stopPropagation()}
        >
          <SelectPrimitive.ScrollUpButton className="asb-select-scroll">
            <ChevronUpIcon />
          </SelectPrimitive.ScrollUpButton>
          <SelectPrimitive.Viewport className="asb-select-viewport">
            {options.map(({ value: optionValue, label, title }) => (
              <SelectPrimitive.Item key={optionValue} value={optionValue} title={title ?? label}
                aria-label={title ?? label} className="asb-select-item">
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
            <ChevronDownIcon />
          </SelectPrimitive.ScrollDownButton>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
