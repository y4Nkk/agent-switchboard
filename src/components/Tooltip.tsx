import { type ReactNode } from "react";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";

interface Props {
  label: ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  children: ReactNode;
}

/**
 * Hover/focus tooltip ported from the spiralcoder reference (Radix Tooltip).
 * Rendering contract and the dark visual are owned here; wrap any element —
 * including disabled buttons — so their gate conditions can be explained.
 */
export function Tooltip({ label, side = "top", open, onOpenChange, children }: Props) {
  return (
    <TooltipPrimitive.Provider delayDuration={150}>
      <TooltipPrimitive.Root open={open} onOpenChange={onOpenChange} disableHoverableContent>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content side={side} sideOffset={6} className="asb-tooltip">
            {label}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
