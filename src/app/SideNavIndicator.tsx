import { useLayoutEffect, useRef, type RefObject } from "react";

/** The lead edge finishes within the first 60% of the travel while the trail
 * edge follows from 35% on, so the pill stretches toward the target before
 * its far side catches up (FlClash's sidebar indicator, prism_ui sidebar.dart). */
const LEAD_PHASE = 0.6;
const TRAIL_PHASE = 0.35;

interface SideNavIndicatorProps {
  list: RefObject<HTMLUListElement | null>;
  /** Index of the current page inside this list; null when the current page
   * lives in the other list, so the pill only ever moves within one list. */
  index: number | null;
}

/** Current-page pill for one sidenav list. CSS paints the geometry
 * (shell.css) and owns the travel duration via --asb-motion-indicator, whose
 * reduce mappings also zero this animation; this component owns the stretch.
 * Item rows are measured, never derived from index arithmetic, so text
 * scaling and list padding cannot desync it. */
export function SideNavIndicator({ list, index }: SideNavIndicatorProps) {
  const pillRef = useRef<HTMLLIElement | null>(null);
  // The last target the pill stretched to; a new selection departs from it.
  const fromRef = useRef<number | null>(index);
  const indexRef = useRef<number | null>(index);
  const frameRef = useRef(0);

  useLayoutEffect(() => {
    if (index === null) {
      indexRef.current = null;
      return;
    }
    const pill = pillRef.current;
    if (!pill) {
      return;
    }
    const from = fromRef.current;
    fromRef.current = index;
    indexRef.current = index;

    const { duration, pillHeight } = indicatorMotion();
    if (pillHeight <= 0) {
      return;
    }
    const apply = (top: number, height: number) => {
      pill.style.top = `${top}px`;
      pill.style.height = `${height}px`;
    };
    const fromTop = from === null ? null : restingTop(list.current, from, pillHeight);
    const toTop = restingTop(list.current, index, pillHeight);
    if (toTop === null) {
      return;
    }
    // Mount (from === index), no readable row, or reduced motion: land
    // directly instead of running a zero-distance stretch.
    if (from === index || fromTop === null || duration <= 0) {
      apply(toTop, pillHeight);
      return;
    }
    const start = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const lead = easeOutCubic(Math.min(1, t / LEAD_PHASE));
      const trail = easeInOutCubic(
        Math.max(0, (t - TRAIL_PHASE) / (1 - TRAIL_PHASE)),
      );
      // Descending: the bottom edge leads; ascending: the top edge does.
      const top =
        toTop >= fromTop
          ? lerp(fromTop, toTop, trail)
          : lerp(fromTop, toTop, lead);
      const bottom =
        toTop >= fromTop
          ? lerp(fromTop, toTop, lead)
          : lerp(fromTop, toTop, trail);
      apply(top, bottom - top + pillHeight);
      if (t < 1) {
        frameRef.current = requestAnimationFrame(step);
      }
    };
    frameRef.current = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frameRef.current);
  }, [index, list]);

  // Layout shifts (rail collapse, font scaling) re-sync the resting pill
  // instantly; the initial observer callback only re-places what mount
  // already placed, so it never interrupts a running stretch.
  useLayoutEffect(() => {
    const l = list.current;
    if (!l) {
      return;
    }
    const observer = new ResizeObserver(() => {
      const current = indexRef.current;
      const pill = pillRef.current;
      if (!pill || current === null) {
        return;
      }
      const { pillHeight } = indicatorMotion();
      const top = restingTop(l, current, pillHeight);
      if (top !== null && pillHeight > 0) {
        pill.style.top = `${top}px`;
        pill.style.height = `${pillHeight}px`;
      }
    });
    observer.observe(l);
    return () => observer.disconnect();
  }, [list]);

  if (index === null) {
    return null;
  }
  return <li ref={pillRef} className="asb-sidenav-indicator" aria-hidden="true" />;
}

/** Duration and pill height resolve from tokens at trigger time, so the
 * reduce mappings in tokens.css gate this JS animation like every CSS one. */
function indicatorMotion() {
  const root = getComputedStyle(document.documentElement);
  return {
    duration: Number.parseFloat(root.getPropertyValue("--asb-motion-indicator")) || 0,
    pillHeight:
      Number.parseFloat(root.getPropertyValue("--asb-sidebar-indicator-height")) || 0,
  };
}

/** The pill's resting top for one row, in the list's content coordinates so
 * absolute positioning inside the scrolling list stays scroll-independent. */
function restingTop(list: HTMLElement | null, index: number, pillHeight: number) {
  if (!list) {
    return null;
  }
  const item = list.querySelectorAll<HTMLElement>(
    ":scope > li .asb-sidenav-item",
  )[index];
  if (!item) {
    return null;
  }
  const rect = item.getBoundingClientRect();
  const top = rect.top - list.getBoundingClientRect().top + list.scrollTop;
  return top + (rect.height - pillHeight) / 2;
}

function lerp(from: number, to: number, t: number) {
  return from + (to - from) * t;
}

function easeOutCubic(t: number) {
  return 1 - (1 - t) ** 3;
}

function easeInOutCubic(t: number) {
  return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
}
