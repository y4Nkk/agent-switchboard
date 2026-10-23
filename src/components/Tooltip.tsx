import { useEffect, type ReactNode, useRef, useState } from "react";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";

interface Props {
  label: ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  className?: string;
  children: ReactNode;
}

/**
 * Hover/focus tooltip for an interactive control. The anchor span owned here
 * is the Radix trigger, not the control itself: Chromium drops pointer events
 * from disabled controls entirely (see tooltip-feedback.css), so a disabled
 * button could neither keep the leave that closes the label nor deliver the
 * press that dismisses it. Activating the control dismisses the label; the
 * dismissal holds until the pointer leaves the anchor or keyboard navigation
 * returns to it. Programmatic focus restore (a dialog handing focus back)
 * counts as neither, so it stays suppressed. Window deactivation dispatches
 * none of these events, so the blur listener closes the label too.
 */
export function Tooltip({ label, side = "top", className, children }: Props) {
  const [open, setOpen] = useState(false);
  const dismissedAfterAction = useRef(false);

  useEffect(() => {
    const close = () => setOpen(false);
    window.addEventListener("blur", close);
    return () => window.removeEventListener("blur", close);
  }, []);

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
          <span className="asb-tooltip-anchor">{children}</span>
        </TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content side={side} sideOffset={6} className={["asb-tooltip", className].filter(Boolean).join(" ")}>
            {label}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
