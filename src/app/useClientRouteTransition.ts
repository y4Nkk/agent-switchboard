import { useLayoutEffect, useMemo, useRef, useState, type RefCallback } from "react";
import type { AppKind } from "../api/client";
import { readSlideMotion } from "../lib/slide-motion";

type RouteFrame = { opacity: string; transform: string };
interface ClientRouteSelection {
  from: AppKind;
  to: AppKind;
  frames: Partial<Record<AppKind, RouteFrame>>;
  focusPicker: boolean;
}
interface RunningClientRoute {
  app: AppKind;
  element: HTMLElement;
  animation: Animation;
}

function animateClientRoute(workspace: HTMLElement, contents: Record<AppKind, HTMLElement | null>,
  selection: ClientRouteSelection): RunningClientRoute[] {
  const outgoing = contents[selection.from];
  const incoming = contents[selection.to];
  if (!outgoing || !incoming) return [];
  const motion = readSlideMotion(workspace);
  if (motion.duration === 0) return [];
  const direction = selection.to === "claude" ? 1 : -1;
  const options: KeyframeAnimationOptions = {
    duration: motion.duration,
    easing: motion.easing,
    fill: "both",
  };
  return [
    {
      app: selection.from,
      element: outgoing,
      animation: outgoing.animate([
        selection.frames[selection.from] ?? { opacity: "1", transform: "translateX(0px)" },
        { opacity: "0", transform: `translateX(${-direction * motion.distance}px)` },
      ], options),
    },
    {
      app: selection.to,
      element: incoming,
      animation: incoming.animate([
        selection.frames[selection.to] ?? { opacity: "0", transform: `translateX(${direction * motion.distance}px)` },
        { opacity: "1", transform: "translateX(0px)" },
      ], options),
    },
  ];
}

/** Owns only picker-triggered visual transitions, never client selection or drafts. */
export function useClientRouteTransition({ app, active, onSelectApp }: {
  app: AppKind;
  active: boolean;
  onSelectApp: (app: AppKind) => void;
}) {
  const workspaceRef = useRef<HTMLDivElement>(null);
  const contents = useRef<Record<AppKind, HTMLElement | null>>({ codex: null, claude: null });
  const running = useRef<RunningClientRoute[]>([]);
  const [selection, setSelection] = useState<ClientRouteSelection | null>(null);
  const contentRefs = useMemo<Record<AppKind, RefCallback<HTMLElement>>>(() => {
    const register = (client: AppKind, element: HTMLElement | null) => {
      const previous = contents.current[client];
      contents.current[client] = element;
      if (previous && !element && running.current.some((route) => route.element === previous)) {
        running.current.forEach(({ animation }) => animation.cancel());
        setSelection(null);
      }
    };
    return {
      codex: (element) => register("codex", element),
      claude: (element) => register("claude", element),
    };
  }, []);
  const leavingApp = active && selection?.to === app ? selection?.from ?? null : null;
  useLayoutEffect(() => {
    if (!selection) return;
    const workspace = workspaceRef.current;
    if (!active || selection.to !== app || !workspace) {
      setSelection(null);
      return;
    }
    if (selection.focusPicker) {
      workspace.querySelector<HTMLInputElement>(
        `[data-client-route="${app}"] .asb-client-picker input:checked`,
      )?.focus({ preventScroll: true });
    }
    const animations = animateClientRoute(workspace, contents.current, selection);
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
  }, [active, app, selection]);
  const selectApp = (next: AppKind) => {
    if (next === app) return;
    const frames: ClientRouteSelection["frames"] = {};
    for (const route of running.current) {
      const styles = getComputedStyle(route.element);
      frames[route.app] = { opacity: styles.opacity, transform: styles.transform };
    }
    running.current.forEach(({ animation }) => animation.cancel());
    const focused = document.activeElement;
    const focusPicker = focused instanceof HTMLInputElement &&
      workspaceRef.current?.contains(focused) === true && focused.closest(".asb-client-picker") !== null;
    setSelection(active ? { from: app, to: next, frames, focusPicker } : null);
    onSelectApp(next);
  };
  return { workspaceRef, contentRefs, leavingApp, selectApp };
}
