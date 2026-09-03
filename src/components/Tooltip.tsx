import { type ReactNode, useRef, useState } from "react";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";

interface Props {
  label: ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  children: ReactNode;
}

/**
 * Hover/focus tooltip for an interactive control. Activating the control
 * dismisses the label; the dismissal holds until the pointer leaves the
 * trigger or keyboard navigation returns to it. Programmatic focus restore
 * (a dialog handing focus back) counts as neither, so it stays suppressed.
 */
export function Tooltip({ label, side = "top", children }: Props) {
  const [open, setOpen] = useState(false);
  const dismissedAfterAction = useRef(false);

  return (
    <TooltipPrimitive.Provider delayDuration={150}>
      <TooltipPrimitive.Root
        open={open}
        onOpenChange={(nextOpen) => {
          if (nextOpen && dismissedAfterAction.current) return;
          setOpen(nextOpen);
        }}
        disableHoverableContent
      >
        <TooltipPrimitive.Trigger
          asChild
          onPointerLeave={() => {
            dismissedAfterAction.current = false;
          }}
          onFocus={(event) => {
            // A focus event carrying a relatedTarget is user navigation
            // (Tab/Shift+Tab); bare focus is programmatic restore.
            if (event.relatedTarget !== null) dismissedAfterAction.current = false;
          }}
          onClickCapture={() => {
            dismissedAfterAction.current = true;
            setOpen(false);
          }}
        >
          {children}
        </TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content side={side} sideOffset={6} className="asb-tooltip">
            {label}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
