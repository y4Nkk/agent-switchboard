import { useId } from "react";
import type { RouteCardStatus } from "./model";

const CONNECTED_TRACE = "M49 32c11 3 14 13 24 13 10 0 12-13 24-16";
const OPEN_TRACE = "M49 32c9 3 12 10 18 12m14-1c5-3 8-11 16-14";

export function RouteCardArt({ status }: { status: RouteCardStatus }) {
  const id = useId();
  const assigned = status === "assigned";
  return <svg className="asb-purpose-card-art" viewBox="0 0 144 64" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-ceramic`} x2="0" y2="1">
        <stop className="asb-route-art-ceramic-light" />
        <stop offset="1" className="asb-route-art-ceramic-shadow" />
      </linearGradient>
      <linearGradient id={`${id}-glass`} x2="1" y2="1">
        <stop className="asb-route-art-glass-light" />
        <stop offset="1" className="asb-route-art-glass-shade" />
      </linearGradient>
    </defs>
    <path className="asb-route-art-floor" d="M5 44 51 11q4-3 9-2l76 17q5 1 2 5L94 60q-5 3-10 2L8 48q-5-1-3-4Z" />
    <path className="asb-route-art-guide" d="m26 51 19-13m23 20 21-15m28-22-16 12" />
    <path className="asb-route-art-base" d="M9 29v5q0 3 4 4l24 7q4 1 7-1l16-11q3-2 3-5v-4Z" />
    <path className="asb-route-art-shell" fill={`url(#${id}-ceramic)`}
      d="m11 26 19-13q3-2 7-1l23 6q5 2 1 6L43 38q-3 2-7 1l-23-7q-6-2-2-6Z" />
    <path className="asb-route-art-glass" fill={`url(#${id}-glass)`}
      d="m22 25 11-8q2-1 4 0l14 4q3 1 1 3l-11 8q-2 1-4 0l-14-4q-3-1-1-3Z" />
    <path className="asb-route-art-port" d="m29 26 5-4 5 1-5 4Zm12-3 4 1-5 4-4-1Z" />
    <path className="asb-route-art-base" d="M87 25v5q0 3 4 4l23 7q4 1 7-1l16-11q3-2 3-5v-4Z" />
    <path className="asb-route-art-shell" fill={`url(#${id}-ceramic)`}
      d="m89 22 19-13q3-2 7-1l22 6q5 2 1 6l-18 14q-3 2-7 1l-22-7q-6-2-2-6Z" />
    <path className="asb-route-art-glass" fill={`url(#${id}-glass)`}
      d="m97 21 13-9q2-1 4 0l15 4q3 1 1 3l-12 9q-2 1-4 0l-15-4q-3-1-2-3Z" />
    <path className="asb-route-art-highlight" d="m104 21 8-6 10 3m-10 4 6 2" />
    <path className="asb-route-art-trace" d={assigned ? CONNECTED_TRACE : OPEN_TRACE} />
    <circle className="asb-route-art-terminal" cx="49" cy="32" r="2.4" />
    <circle className="asb-route-art-terminal" cx="97" cy="29" r="2.4" />
    {status === "needsAttention" && <path className="asb-route-art-trace" d="m72 41 6 6m0-6-6 6" />}
  </svg>;
}
