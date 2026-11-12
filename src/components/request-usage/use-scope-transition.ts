import { useLayoutEffect, useRef, useState } from "react";
import type { MeasurementScope, RequestUsageReport } from "../../api/client";
import { readSlideMotion } from "../../lib/slide-motion";

/** Segment order is generation → transport; slides follow the segment side. */
const DIRECTION: Record<MeasurementScope, 1 | -1> = { generation: -1, transport: 1 };

interface ExitLayer { report: RequestUsageReport; direction: 1 | -1 }

/**
 * Owns the scope-switch slide for the performance stage, mirroring the client
 * route transition: when the segment flips, the previous report renders once
 * more as an inert exit layer that slides out while the incoming report, once
 * it arrives, slides in. The exit layer is a read-only snapshot kept only for
 * the animation; switching back mid-exit cancels it.
 */
export function useScopeTransition(report: RequestUsageReport | null, scope: MeasurementScope) {
  const [exit, setExit] = useState<ExitLayer | null>(null);
  const latest = useRef<RequestUsageReport | null>(null);
  const previousScope = useRef(scope);
  const armed = useRef<MeasurementScope | null>(null);
  const running = useRef<Animation[]>([]);
  const stageRef = useRef<HTMLDivElement>(null);
  const exitLayerRef = useRef<HTMLElement | null>(null);
  const currentLayerRef = useRef<HTMLElement | null>(null);

  useLayoutEffect(() => {
    if (report) latest.current = report;
  }, [report]);

  useLayoutEffect(() => {
    if (previousScope.current === scope) return;
    previousScope.current = scope;
    running.current.forEach((animation) => animation.cancel());
    running.current = [];
    if (exit && exit.report.scope === scope) {
      setExit(null);
      armed.current = null;
      return;
    }
    const previous = latest.current;
    if (previous && previous.scope !== scope) {
      armed.current = scope;
      setExit({ report: previous, direction: DIRECTION[scope] });
    } else {
      armed.current = null;
    }
  }, [scope, exit]);

  useLayoutEffect(() => {
    const layer = exitLayerRef.current;
    const stage = stageRef.current;
    if (!exit || !layer || !stage) return;
    const motion = readSlideMotion(stage);
    if (motion.duration === 0) {
      setExit(null);
      return;
    }
    const animation = layer.animate([
      { opacity: "1", transform: "translateX(0px)" },
      { opacity: "0", transform: `translateX(${-exit.direction * motion.distance}px)` },
    ], { duration: motion.duration, easing: motion.easing, fill: "both" });
    running.current.push(animation);
    let settled = false;
    const settle = () => {
      if (!settled) setExit((current) => current === exit ? null : current);
    };
    void animation.finished.then(settle, settle);
    return () => {
      settled = true;
      animation.cancel();
      running.current = running.current.filter((item) => item !== animation);
    };
  }, [exit]);

  useLayoutEffect(() => {
    if (!report || armed.current !== report.scope) return;
    armed.current = null;
    const layer = currentLayerRef.current;
    const stage = stageRef.current;
    if (!layer || !stage) return;
    const motion = readSlideMotion(stage);
    if (motion.duration === 0) return;
    const direction = DIRECTION[report.scope];
    const animation = layer.animate([
      { opacity: "0", transform: `translateX(${direction * motion.distance}px)` },
      { opacity: "1", transform: "translateX(0px)" },
    ], { duration: motion.duration, easing: motion.easing, fill: "both" });
    return () => animation.cancel();
  }, [report]);

  return {
    stageRef, exit,
    bindExitLayer: (element: HTMLElement | null) => { exitLayerRef.current = element; },
    bindCurrentLayer: (element: HTMLElement | null) => { currentLayerRef.current = element; },
  };
}
