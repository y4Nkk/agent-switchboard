import { useId, useImperativeHandle, useRef, type HTMLAttributes, type ReactNode, type Ref, type UIEventHandler } from "react";
import { BackToTop } from "./BackToTop";

interface Props extends Omit<HTMLAttributes<HTMLElement>, "onScroll"> {
  as?: "div" | "main" | "section";
  /** Reading-position owners reference the actual scrolling element. */
  viewportRef?: Ref<HTMLDivElement>;
  onScroll?: UIEventHandler<HTMLDivElement>;
  floatingActions?: { content: ReactNode; label?: string };
}

/** The frame owns BackToTop. Floating actions stay in the viewport's flow
 * so its final content can scroll clear of the footer. */
export function ScrollArea({ as: Tag = "div", className, viewportRef, onScroll, floatingActions,
  tabIndex = -1, children, ...props }: Props) {
  const viewport = useRef<HTMLDivElement>(null);
  const id = useId();
  useImperativeHandle(viewportRef, () => viewport.current!, []);
  return (
    <Tag {...props} className={["asb-scroll-area", className].filter(Boolean).join(" ")}
      data-floating-actions={floatingActions ? "" : undefined}>
      <div id={id} className="asb-scroll-viewport" ref={viewport} tabIndex={tabIndex}
        aria-label={props["aria-label"]} onScroll={onScroll}>
        {children}
        {floatingActions && <footer className="asb-scroll-action-hub" aria-label={floatingActions.label}>
          {floatingActions.content}
        </footer>}
      </div>
      <BackToTop viewport={viewport} controls={id} />
    </Tag>
  );
}
