import { useLayoutEffect, useMemo, useRef, useState, type RefCallback } from "react";
import { readSlideMotion } from "../lib/slide-motion";

type SlideFrame = { opacity: string; transform: string };
interface PanelSelection<T extends string> {
  from: T;
  to: T;
  frames: ReadonlyMap<T, SlideFrame>;
  focusIncoming?: (stage: HTMLElement) => void;
}
interface RunningSlide<T extends string> {
  panel: T;
  element: HTMLElement;
  animation: Animation;
}

function animatePanels<T extends string>(stage: HTMLElement, panels: ReadonlyMap<T, HTMLElement>,
  { from, to, frames }: PanelSelection<T>, direction: number): RunningSlide<T>[] {
  const outgoing = panels.get(from);
  const incoming = panels.get(to);
  if (!outgoing || !incoming) return [];
  const motion = readSlideMotion(stage);
  if (motion.duration === 0) return [];
  const options: KeyframeAnimationOptions = {
    duration: motion.duration,
    easing: motion.easing,
    fill: "both",
  };
  return [
    {
      panel: from,
      element: outgoing,
      animation: outgoing.animate([
        frames.get(from) ?? { opacity: "1", transform: "translateX(0px)" },
        { opacity: "0", transform: `translateX(${-direction * motion.distance}px)` },
      ], options),
    },
    {
      panel: to,
      element: incoming,
      animation: incoming.animate([
        frames.get(to) ?? { opacity: "0", transform: `translateX(${direction * motion.distance}px)` },
        { opacity: "1", transform: "translateX(0px)" },
      ], options),
    },
  ];
}

/** Animates explicit selections. The caller owns the selected value and the panels' lifetimes. */
export function usePanelSlideTransition<T extends string>({ value, order, active, onChange }: {
  value: T;
  order: readonly T[];
  active: boolean;
  onChange: (value: T) => void;
}) {
  const stageRef = useRef<HTMLDivElement>(null);
  const panels = useRef(new Map<T, HTMLElement>());
  const running = useRef<RunningSlide<T>[]>([]);
  const [selection, setSelection] = useState<PanelSelection<T> | null>(null);
  const panelRefs = useMemo(() => {
    const refs = {} as Record<T, RefCallback<HTMLElement>>;
    for (const panel of order) {
      refs[panel] = (element) => {
        if (element) {
          panels.current.set(panel, element);
          return;
        }
        const previous = panels.current.get(panel);
        panels.current.delete(panel);
        if (running.current.some((slide) => slide.element === previous)) {
          running.current.forEach(({ animation }) => animation.cancel());
          setSelection(null);
        }
      };
    }
    return refs;
  }, [order]);
  const leaving = active && selection && selection.to === value ? selection.from : null;
  useLayoutEffect(() => {
    if (!selection) return;
    const stage = stageRef.current;
    if (!active || selection.to !== value || !stage) {
      setSelection(null);
      return;
    }
    selection.focusIncoming?.(stage);
    const direction = order.indexOf(selection.to) > order.indexOf(selection.from) ? 1 : -1;
    const animations = animatePanels(stage, panels.current, selection, direction);
    running.current = animations;
    let cancelled = false;
    const settle = () => {
      if (!cancelled) setSelection((current) => current === selection ? null : current);
    };
    if (animations.length === 0) settle();
    else void Promise.allSettled(animations.map(({ animation }) => animation.finished)).then(settle);
    return () => {
      cancelled = true;
      animations.forEach(({ animation }) => animation.cancel());
      running.current = [];
    };
  }, [active, order, selection, value]);
  const select = (next: T, focusIncoming?: (stage: HTMLElement) => void) => {
    if (next === value) return;
    const frames = new Map<T, SlideFrame>();
    for (const slide of running.current) {
      const styles = getComputedStyle(slide.element);
      frames.set(slide.panel, { opacity: styles.opacity, transform: styles.transform });
    }
    running.current.forEach(({ animation }) => animation.cancel());
    setSelection(active ? { from: value, to: next, frames, focusIncoming } : null);
    onChange(next);
  };
  return { stageRef, panelRefs, leaving, select };
}
