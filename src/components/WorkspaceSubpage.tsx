import { useEffect, useImperativeHandle, useRef, useState, type ReactElement, type ReactNode, type Ref, type RefObject } from "react";
import { Button } from "./Button";
import { WorkspaceHeader } from "./WorkspaceHeader";
import { ScrollArea } from "./ScrollArea";

type Props = {
  title: string;
  titleRef?: Ref<HTMLHeadingElement>;
  returnFocusTarget?: Element | null;
  returnFocusRef?: RefObject<HTMLElement | null>;
  viewportRef?: Ref<HTMLDivElement>;
  backLabel: string;
  busy?: boolean;
  primary?: ReactNode;
  onBack: () => void;
  className?: string;
  children: ReactNode;
} & (
  | { layout: "page"; footer?: never }
  | { layout: "fixed-actions" | "floating-actions"; footer: ReactElement }
);

export function WorkspaceSubpage({
  layout,
  title,
  titleRef,
  returnFocusTarget,
  returnFocusRef,
  viewportRef,
  backLabel,
  busy = false,
  primary,
  onBack,
  className,
  children,
  footer,
}: Props) {
  const subpage = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  useImperativeHandle(titleRef, () => heading.current!, []);
  const [returnTarget] = useState(() => returnFocusTarget ?? document.activeElement);
  useEffect(() => {
    const target = heading.current;
    const frame = subpage.current;
    if (!target || !frame || target.closest("[hidden]")) return;
    if (!frame.contains(document.activeElement)) target.focus({ preventScroll: true });
    return () => {
      const active = document.activeElement;
      if (active !== document.body && !frame.contains(active)) return;
      for (const candidate of [returnTarget, returnFocusRef?.current]) {
        if (!(candidate instanceof HTMLElement) || candidate === document.body ||
          !candidate.isConnected || !candidate.getClientRects().length) continue;
        candidate.focus({ preventScroll: true });
        if (document.activeElement === candidate) return;
      }
    };
  }, [returnFocusRef, returnTarget]);
  const header = (
    <WorkspaceHeader title={title} titleRef={heading} primary={primary} back={
      <Button variant="back" disabled={busy} aria-label={backLabel} onClick={onBack}>
        <span aria-hidden="true">←</span>
      </Button>
    } />
  );
  const fixedHeader = layout === "fixed-actions";
  return (
    <div ref={subpage} className={["asb-workspace-subpage", className].filter(Boolean).join(" ")}>
      {fixedHeader && header}
      <ScrollArea className="asb-subpage-scroll" aria-label={title} viewportRef={viewportRef}
        floatingActions={layout === "floating-actions" ? { content: footer } : undefined}>
        {!fixedHeader && header}
        {children}
      </ScrollArea>
      {fixedHeader && <footer className="asb-subpage-footer">{footer}</footer>}
    </div>
  );
}
