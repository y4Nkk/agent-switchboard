import { useEffect, useRef, useState, type ReactNode, type Ref } from "react";
import { Button } from "./Button";
import { WorkspaceHeader } from "./WorkspaceHeader";
import { ScrollArea } from "./ScrollArea";

interface Props {
  title: string;
  titleRef?: Ref<HTMLHeadingElement>;
  /** Reading-position owners reference the actual scrolling element. */
  viewportRef?: Ref<HTMLDivElement>;
  /** Destination name, carried as the back control's accessible label. */
  backLabel: string;
  busy?: boolean;
  /** Header row 2 context (tabs, provider name) beside the back control. */
  primary?: ReactNode;
  onBack: () => void;
  /** Surface modifier for caller-specific visuals, scoped below the frame contract. */
  className?: string;
  children: ReactNode;
  /** Fixed bottom action bar; its content is owned by the composing surface.
   * Surfaces without a persistent commit action omit it. */
  footer?: ReactNode;
}

/** Owns the full-page editor frame: a pinned header row with the back
 * control, a scrollable module body, and a pinned bottom action bar. */
export function EditorFrame({
  title,
  titleRef,
  viewportRef,
  backLabel,
  busy = false,
  primary,
  onBack,
  className,
  children,
  footer,
}: Props) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [returnTarget] = useState(() => document.activeElement);
  useEffect(() => {
    // Callers with a title ref already own route-specific focus changes.
    if (titleRef) return;
    const target = heading.current;
    const frame = target?.closest(".asb-editor-frame");
    if (!target || !frame || target.closest("[hidden]")) return;
    if (!frame.contains(document.activeElement)) target.focus({ preventScroll: true });
    return () => {
      const active = document.activeElement;
      if (active !== document.body && !frame.contains(active)) return;
      if (returnTarget instanceof HTMLElement && returnTarget.isConnected && returnTarget.getClientRects().length > 0)
        returnTarget.focus({ preventScroll: true });
    };
  }, [titleRef, returnTarget]);
  return (
    <div className={className ? `asb-editor-frame ${className}` : "asb-editor-frame"}>
      <WorkspaceHeader
        title={title}
        titleRef={titleRef ?? heading}
        back={
          <Button variant="back" disabled={busy} aria-label={backLabel} onClick={onBack}>
            <span aria-hidden="true">←</span>
          </Button>
        }
        primary={primary}
      />
      <ScrollArea className="asb-edit-panel" aria-label={title} viewportRef={viewportRef}>{children}</ScrollArea>
      {footer && <footer className="asb-editor-footer">{footer}</footer>}
    </div>
  );
}
