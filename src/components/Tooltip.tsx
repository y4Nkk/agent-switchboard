import { createContext, useContext, useEffect, useRef, useState, type ReactElement, type ReactNode,
  type RefObject } from "react";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";

type Props = {
  label: ReactNode;
  enabled?: boolean;
  side?: "top" | "bottom" | "left" | "right";
  className?: string;
  onEscapeKeyDown?: (event: KeyboardEvent) => void;
} & ({ asChild: true; children: ReactElement } | { asChild?: false; children: ReactNode });

export const TooltipPortalContext = createContext<RefObject<HTMLElement | null> | null>(null);
const DISMISS_EVENT = "asb-tooltip-dismiss";

export function dismissTooltips() {
  window.dispatchEvent(new Event(DISMISS_EVENT));
}

/**
 * Hover/focus tooltip. The default anchor span owns hit testing for controls
 * that can be disabled: Chromium drops pointer events
 * from disabled controls entirely (see tooltip-feedback.css), so a disabled
 * button could neither keep the leave that closes the label nor deliver the
 * press that dismisses it. Activating the control dismisses the label; the
 * dismissal holds until the pointer leaves the anchor or keyboard navigation
 * returns to it. Programmatic focus restore (a dialog handing focus back)
 * counts as neither, so it stays suppressed. Window deactivation dispatches
 * none of these events, so the blur listener closes the label too.
 */
export function Tooltip({ label, enabled = true, side = "top", className, asChild, onEscapeKeyDown, children }: Props) {
  const [open, setOpen] = useState(false);
  const dismissedAfterAction = useRef(false);
  const container = useContext(TooltipPortalContext);

  useEffect(() => {
    if (!enabled) setOpen(false);
  }, [enabled]);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    window.addEventListener("blur", close);
    window.addEventListener(DISMISS_EVENT, close);
    return () => {
      window.removeEventListener("blur", close);
      window.removeEventListener(DISMISS_EVENT, close);
    };
  }, [open]);

  return (
    <TooltipPrimitive.Provider delayDuration={150}>
      <TooltipPrimitive.Root
        open={enabled && open}
        onOpenChange={(nextOpen) => {
          if (nextOpen && (!enabled || dismissedAfterAction.current || document.hidden)) return;
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
          {asChild ? children : <span className="asb-tooltip-anchor">{children}</span>}
        </TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal container={container?.current}>
          <TooltipPrimitive.Content side={side} sideOffset={6} onEscapeKeyDown={onEscapeKeyDown}
            style={{ pointerEvents: "none" }} className={["asb-tooltip", className].filter(Boolean).join(" ")}>
            {label}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
