import { useEffect, useId, useRef, useState, type RefObject } from "react";
import type { RouteRole } from "./model";

interface Path { key: string; role: RouteRole; d: string }

/** Coordinates belong to rendered node edges, including wrapped text and app scaling. */
export function RouteConnections({ scene, roles, selected }: {
  scene: RefObject<HTMLDivElement | null>; roles: RouteRole[]; selected: RouteRole;
}) {
  const [paths, setPaths] = useState<Path[]>([]);
  const frame = useRef<number | null>(null);
  const arrow = useId();
  const key = roles.join(":");
  // The parent scene ref is attached after child layout effects finish.
  useEffect(() => {
    const root = scene.current;
    if (!root) return;
    const measure = () => {
      frame.current = null;
      const bounds = root.getBoundingClientRect();
      const node = (id: string) => root.querySelector<HTMLElement>(`[data-route-node="${id}"]`)?.getBoundingClientRect();
      const next: Path[] = [];
      for (const role of roles) {
        for (const [start, end] of [["client", `purpose-${role}`], [`purpose-${role}`, `target-${role}`]]) {
          const a = node(start);
          const b = node(end);
          if (!a || !b || b.left <= a.right) continue;
          const x = a.right - bounds.left;
          const y = a.top + a.height / 2 - bounds.top;
          const targetX = b.left - bounds.left;
          const targetY = b.top + b.height / 2 - bounds.top;
          const bend = (targetX - x) / 2;
          next.push({ key: `${start}:${end}`, role,
            d: `M ${x} ${y} C ${x + bend} ${y}, ${targetX - bend} ${targetY}, ${targetX} ${targetY}` });
        }
      }
      setPaths((previous) => JSON.stringify(previous) === JSON.stringify(next) ? previous : next);
    };
    const schedule = () => { if (frame.current === null) frame.current = requestAnimationFrame(measure); };
    const observer = new ResizeObserver(schedule);
    observer.observe(root);
    root.querySelectorAll<HTMLElement>("[data-route-node]").forEach((node) => observer.observe(node));
    schedule();
    return () => { observer.disconnect(); if (frame.current !== null) cancelAnimationFrame(frame.current); frame.current = null; };
  }, [scene, key]);
  return <svg className="asb-model-route-wires" aria-hidden="true" focusable="false">
    <defs><marker id={arrow} markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M 0 0 L 7 3.5 L 0 7 Z" fill="context-stroke" />
    </marker></defs>
    {paths.map((path) => <path key={path.key} d={path.d} markerEnd={`url(#${arrow})`}
      className={path.role === selected ? "is-selected" : undefined} />)}
  </svg>;
}
