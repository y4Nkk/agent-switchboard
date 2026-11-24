import { useId, useLayoutEffect, useState } from "react";
import type { AppKind, GatewayRouteStatus } from "../api/client";
import { NATIVE_PROTOCOL, isProtocolTranslation } from "../lib/protocol";
import type { ClientFilterValue } from "./ClientFilter";

interface Point {
  x: number;
  y: number;
}

type Anchor = Partial<Record<"input" | "output", Point>>;

interface Geometry {
  width: number;
  height: number;
  anchors: Record<string, Anchor>;
}

interface Props {
  scene: HTMLDivElement | null;
  routes: GatewayRouteStatus[];
  focus: ClientFilterValue;
  failed: boolean;
}

/** Wire endpoints follow the actual SVG jacks through layout and zoom. */
export function GatewayTopologyWires({ scene, routes, focus, failed }: Props) {
  const markerId = `gateway-direction-${useId().replace(/:/g, "")}`;
  const geometry = useDeviceGeometry(scene, routes);
  const { input: hubInput, output: hubOutput } = geometry.anchors.hub ?? {};
  if (!hubInput || !hubOutput || geometry.width === 0 || geometry.height === 0) return null;
  const clients = [...new Set(routes.map((route) => route.app))];
  return (
    <svg className={`asb-gateway-wires${failed ? " is-failure" : ""}`}
      viewBox={`0 0 ${geometry.width} ${geometry.height}`} aria-hidden="true" focusable="false">
      <defs>
        <marker id={markerId} viewBox="0 0 10 10" refX="9" refY="5"
          markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto">
          <path d="M1.5 2 8 5 1.5 8" fill="none" stroke="context-stroke" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round" />
        </marker>
      </defs>
      {clients.map((app) => {
        const output = geometry.anchors[`client:${app}`]?.output;
        return output && <Wire key={app} app={app} focus={focus} markerId={markerId} from={output} to={hubInput} />;
      })}
      {routes.map((route) => {
        const input = geometry.anchors[`${route.app}:${route.profileId}`]?.input;
        return input && <Wire key={`${route.app}:${route.profileId}`} app={route.app} focus={focus} markerId={markerId}
          translated={isProtocolTranslation(NATIVE_PROTOCOL[route.app], route.upstreamProtocol)}
          from={hubOutput} to={input} />;
      })}
    </svg>
  );
}

function Wire({ from, to, app, focus, markerId, translated = false }: {
  from: Point;
  to: Point;
  app: AppKind;
  focus: ClientFilterValue;
  markerId: string;
  translated?: boolean;
}) {
  const bend = (to.x - from.x) * .53;
  const path = `M${from.x} ${from.y} C${from.x + bend} ${from.y} ${to.x - bend} ${to.y} ${to.x} ${to.y}`;
  return <g className={`asb-gateway-wire${translated ? " is-translation" : ""}${focus !== "all" && focus !== app ? " is-muted" : ""}`}>
    <path d={path} className="asb-gateway-wire-halo" />
    <path d={path} className="asb-gateway-wire-stroke" markerEnd={`url(#${markerId})`} />
  </g>;
}

function useDeviceGeometry(scene: HTMLDivElement | null, routes: GatewayRouteStatus[]) {
  const [geometry, setGeometry] = useState<Geometry>({ width: 0, height: 0, anchors: {} });
  useLayoutEffect(() => {
    if (!scene) return;
    const nodes = [...scene.querySelectorAll<HTMLElement>("[data-gateway-node]")];
    const measure = () => {
      const bounds = scene.getBoundingClientRect();
      const anchors: Record<string, Anchor> = {};
      for (const node of nodes) {
        const key = node.dataset.gatewayNode;
        if (!key) continue;
        const anchor: Anchor = {};
        for (const port of node.querySelectorAll("[data-gateway-port]")) {
          const direction = port.getAttribute("data-gateway-port");
          if (direction !== "input" && direction !== "output") continue;
          const rect = port.getBoundingClientRect();
          anchor[direction] = {
            x: rect.left - bounds.left + rect.width / 2,
            y: rect.top - bounds.top + rect.height / 2,
          };
        }
        anchors[key] = anchor;
      }
      setGeometry({ width: bounds.width, height: bounds.height, anchors });
    };
    let frame: number | null = null;
    const observer = new ResizeObserver(() => {
      if (frame !== null) window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(measure);
    });
    observer.observe(scene);
    nodes.forEach((node) => observer.observe(node));
    measure();
    return () => {
      observer.disconnect();
      if (frame !== null) window.cancelAnimationFrame(frame);
    };
  }, [scene, routes]);
  return geometry;
}
