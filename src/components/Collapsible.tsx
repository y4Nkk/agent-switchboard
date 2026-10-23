import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type TransitionEvent,
} from "react";
import { Button } from "./Button";
import { ChevronDownIcon } from "./icons";

/**
 * The single expand/collapse primitive (DESIGN.md §8): every disclosure in
 * the app — row panels, editor folds, evidence sections — renders the
 * `.asb-collapsible` grammar owned by styles/base/collapsible.css through
 * this component. No screen keeps its own fold CSS, `<details>` element, or
 * chevron system.
 *
 * Mount contract: content mounts when opening and stays mounted through the
 * collapse transition, then unmounts. Nothing runs or polls while hidden,
 * and closed disclosures occupy no layout. The expand transition needs one
 * collapsed layout pass, so the effect forces a reflow before applying the
 * open class; transition completion and a duration-aware fallback both
 * finish cleanup, covering reduced motion and interrupted transitions.
 */
function useDisclosureFrame(open: boolean, onClosed?: () => void) {
  const [mounted, setMounted] = useState(open);
  const [expanded, setExpanded] = useState(open);
  const frameRef = useRef<HTMLDivElement | null>(null);
  // Read through a ref so the fallback timer effect keeps its [open, mounted]
  // dependencies regardless of how often the callback identity changes.
  const onClosedRef = useRef(onClosed);
  onClosedRef.current = onClosed;
  useLayoutEffect(() => {
    if (open) setMounted(true);
    else setExpanded(false);
  }, [open]);
  useLayoutEffect(() => {
    if (!mounted || !open || expanded) return;
    frameRef.current?.getBoundingClientRect();
    setExpanded(true);
  }, [mounted, open, expanded]);
  useEffect(() => {
    if (open || !mounted) return;
    const frame = frameRef.current;
    const durations = frame ? getComputedStyle(frame).transitionDuration.split(",") : [];
    const delay = Math.max(0, ...durations.map((value) =>
      Number.parseFloat(value) * (value.trim().endsWith("ms") ? 1 : 1000)));
    // Cleanup must also happen with zero motion or an interrupted transition.
    const timer = window.setTimeout(() => {
      setMounted(false);
      onClosedRef.current?.();
    }, delay);
    return () => window.clearTimeout(timer);
  }, [open, mounted]);
  return {
    mounted,
    expanded,
    frameRef,
    onTransitionEnd: (event: TransitionEvent<HTMLDivElement>) => {
      // Collapsed descendants fire their own transitions into this bubble.
      if (expanded || event.target !== event.currentTarget) return;
      if (event.propertyName === "grid-template-rows") {
        setMounted(false);
        onClosedRef.current?.();
      }
    },
  };
}

interface CollapsibleProps {
  open: boolean;
  /** Target of the trigger's aria-controls while the content exists. */
  id?: string;
  /** Promotes the region to a labelled landmark for toolbar-driven panels. */
  ariaLabel?: string;
  /** Fires once when the collapse transition finished and the content
   * unmounts; lets a caller swap replacement content in exactly then. */
  onClosed?: () => void;
  className?: string;
  children: ReactNode;
}

/** The animated disclosure body for triggers owned by the caller (row action
 * buttons, toolbar entries). The trigger declares aria-expanded and, while
 * open, aria-controls pointing at this element's id. */
export function Collapsible({ open, id, ariaLabel, onClosed, className, children }: CollapsibleProps) {
  const { mounted, expanded, frameRef, onTransitionEnd } = useDisclosureFrame(open, onClosed);
  if (!mounted) return null;
  return (
    <div
      ref={frameRef}
      id={id}
      inert={!open}
      role={ariaLabel ? "region" : undefined}
      aria-label={ariaLabel}
      className={["asb-collapsible", expanded ? "is-open" : "", className]
        .filter(Boolean)
        .join(" ")}
      onTransitionEnd={onTransitionEnd}
    >
      <div className="asb-collapsible-content">{children}</div>
    </div>
  );
}

/** The one rotating disclosure chevron; the single icon grammar for every
 * fold, replacing swapped arrow pairs and CSS-generated markers. */
export function CollapsibleChevron({ open }: { open: boolean }) {
  return (
    <span className={`asb-collapsible-chevron${open ? " is-open" : ""}`} aria-hidden="true">
      <ChevronDownIcon />
    </span>
  );
}

interface CollapsibleSectionProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The summary row's text; rich labels keep their own heading elements. */
  label: ReactNode;
  /** Trailing muted fact next to the label (configured count, tier mode). */
  value?: ReactNode;
  /** Secondary evidence tone: muted compact row for per-item folds. */
  quiet?: boolean;
  className?: string;
  children: ReactNode;
}

/** The self-contained disclosure that replaces `<details>`: one summary
 * button (aria-expanded + aria-controls) above the shared animated body.
 * Uncontrolled state belongs to the caller via useState, like every other
 * controlled component in this codebase. */
export function CollapsibleSection({
  open,
  onOpenChange,
  label,
  value,
  quiet,
  className,
  children,
}: CollapsibleSectionProps) {
  const contentId = useId();
  return (
    <div className={["asb-collapsible-section", quiet ? "is-quiet" : "", className]
      .filter(Boolean)
      .join(" ")}>
      <Button
        variant="unstyled"
        className="asb-collapsible-summary"
        aria-expanded={open}
        aria-controls={open ? contentId : undefined}
        onClick={() => onOpenChange(!open)}
      >
        <span className="asb-collapsible-summary-label">{label}</span>
        {value != null && <span className="asb-collapsible-summary-value">{value}</span>}
        <CollapsibleChevron open={open} />
      </Button>
      <Collapsible open={open} id={contentId}>
        <div className="asb-collapsible-body">{children}</div>
      </Collapsible>
    </div>
  );
}
