import { useLayoutEffect, useRef, type RefObject } from "react";
import type { Page } from "./navigation";

interface IndicatorFrame { top: number; bottom: number }
interface IndicatorState {
  frame: IndicatorFrame | null;
  target: IndicatorFrame | null;
  cancel: () => void;
}

export function SideNavIndicator({ list, current }: {
  list: HTMLUListElement | null;
  current: Page | null;
}) {
  const pill = useRef<HTMLLIElement>(null);
  const state = useRef<IndicatorState>({ frame: null, target: null, cancel: () => {} });
  useIndicatorLayout(list, pill, state);

  useLayoutEffect(() => {
    const motion = state.current;
    motion.cancel();
    const target = measureTarget(list);
    const node = pill.current;
    if (!node || !target || current === null) {
      motion.frame = null;
      motion.target = null;
      return;
    }
    const from = motion.frame;
    motion.target = target;
    const duration = readDuration();
    if (!from || duration <= 0 || document.hidden || sameFrame(from, target)) {
      paintFrame(node, motion, target);
      return;
    }
    motion.cancel = animateIndicator(node, motion, from, target, duration);
    return () => motion.cancel();
  }, [current, list]);

  return current === null ? null : (
    <li ref={pill} className="asb-sidenav-indicator" aria-hidden="true" />
  );
}

function useIndicatorLayout(
  list: HTMLUListElement | null,
  pill: RefObject<HTMLLIElement | null>,
  state: RefObject<IndicatorState>,
) {
  useLayoutEffect(() => {
    const container = list;
    if (!container) return;
    const settle = () => {
      const target = measureTarget(container);
      const node = pill.current;
      if (!target || !node) return;
      state.current.cancel();
      state.current.target = target;
      paintFrame(node, state.current, target);
    };
    const resize = new ResizeObserver(() => {
      const next = measureTarget(container);
      const previous = state.current.target;
      // Initial observer delivery and width-only changes must not end travel.
      if (next && (!previous || !sameFrame(next, previous))) settle();
    });
    resize.observe(container);
    container.querySelectorAll(".asb-sidenav-item").forEach((item) => resize.observe(item));
    const preferenceChanged = () => { if (readDuration() <= 0) settle(); };
    const preferences = new MutationObserver(preferenceChanged);
    preferences.observe(document.documentElement, {
      attributes: true, attributeFilter: ["data-motion", "class", "style"],
    });
    const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion.addEventListener("change", preferenceChanged);
    document.addEventListener("visibilitychange", settle);
    window.addEventListener("blur", settle);
    return () => {
      state.current.cancel();
      resize.disconnect();
      preferences.disconnect();
      reducedMotion.removeEventListener("change", preferenceChanged);
      document.removeEventListener("visibilitychange", settle);
      window.removeEventListener("blur", settle);
    };
  }, [list, pill, state]);
}

function animateIndicator(
  node: HTMLElement, state: IndicatorState, from: IndicatorFrame,
  to: IndicatorFrame, duration: number,
) {
  let frame = 0;
  const start = performance.now();
  const descending = (to.top + to.bottom) >= (from.top + from.bottom);
  const step = (now: number) => {
    const progress = Math.min(1, (now - start) / duration);
    const lead = 1 - (1 - Math.min(1, progress / 0.6)) ** 3;
    const trailingProgress = Math.max(0, (progress - 0.35) / 0.65);
    const trail = trailingProgress < 0.5
      ? 4 * trailingProgress ** 3 : 1 - (-2 * trailingProgress + 2) ** 3 / 2;
    paintFrame(node, state, {
      top: lerp(from.top, to.top, descending ? trail : lead),
      bottom: lerp(from.bottom, to.bottom, descending ? lead : trail),
    });
    if (progress < 1) frame = requestAnimationFrame(step);
  };
  frame = requestAnimationFrame(step);
  return () => cancelAnimationFrame(frame);
}

function paintFrame(node: HTMLElement, state: IndicatorState, frame: IndicatorFrame) {
  state.frame = frame;
  node.style.top = `${frame.top}px`;
  node.style.height = `${Math.max(0, frame.bottom - frame.top)}px`;
}

function measureTarget(list: HTMLElement | null): IndicatorFrame | null {
  const item = list?.querySelector<HTMLElement>('.asb-sidenav-item[aria-current="page"]');
  if (!list || !item) return null;
  const height = Number.parseFloat(getComputedStyle(document.documentElement)
    .getPropertyValue("--asb-sidebar-indicator-height"));
  if (!(height > 0)) return null;
  const rect = item.getBoundingClientRect();
  if (item.offsetHeight === 0) return null;
  // Rects include ancestor CSS zoom; top and scrollTop use layout pixels.
  const zoom = rect.height / item.offsetHeight;
  const top = (rect.top - list.getBoundingClientRect().top) / zoom + list.scrollTop
    + (item.offsetHeight - height) / 2;
  return { top, bottom: top + height };
}

function readDuration() {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue("--asb-motion-indicator").trim();
  return Number.parseFloat(value) * (value.endsWith("ms") ? 1 : 1000);
}

function sameFrame(a: IndicatorFrame, b: IndicatorFrame) {
  return Math.abs(a.top - b.top) < 0.01 && Math.abs(a.bottom - b.bottom) < 0.01;
}

function lerp(from: number, to: number, progress: number) {
  return from + (to - from) * progress;
}
