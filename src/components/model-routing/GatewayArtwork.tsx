import { useId } from "react";

const LID = "M105 132 190 76Q199 70 210 70L607 82Q623 82 623 90Q623 94 615 99L535 151Q529 155 517 155L113 143Q91 142 105 132Z";
const SIDE = "M523 150 614 92Q623 86 623 91V167Q623 176 615 181L535 234Q529 238 522 238Z";
const LID_PLANE = "matrix(1 .03 .82 -.58 102 138)";
const FRONT_PLANE = "matrix(1 .03 0 1 102 138)";
const SIDE_PLANE = "matrix(.82 -.55 0 1 527 151)";
const VENTS = Array.from({ length: 16 }, (_, index) => index);

function paint(id: string, layer: string) {
  return `url(#${id}-${layer})`;
}

/** The device material palette; the backup archive device paints with it too. */
export function ChassisPaints({ id }: { id: string }) {
  return <defs>
    <radialGradient id={`${id}-shadow`}>
      <stop stopColor="#263957" stopOpacity=".48" />
      <stop offset=".52" stopColor="#263957" stopOpacity=".25" />
      <stop offset="1" stopColor="#263957" stopOpacity="0" />
    </radialGradient>
    <linearGradient id={`${id}-lid`} x1="15%" y1="0%" x2="87%" y2="99%">
      <stop stopColor="#f4f8fc" />
      <stop offset=".4" stopColor="#c2cfdf" />
      <stop offset=".73" stopColor="#94a7bd" />
      <stop offset="1" stopColor="#d6deec" />
    </linearGradient>
    <linearGradient id={`${id}-frame`} x1="0%" y1="0%" x2="0%" y2="100%">
      <stop stopColor="#aabacd" />
      <stop offset=".17" stopColor="#5c7089" />
      <stop offset=".84" stopColor="#344961" />
      <stop offset="1" stopColor="#899bb1" />
    </linearGradient>
    <linearGradient id={`${id}-side`} x1="0%" y1="0%" x2="100%" y2="75%">
      <stop stopColor="#283b55" />
      <stop offset=".6" stopColor="#425674" />
      <stop offset="1" stopColor="#73839d" />
    </linearGradient>
    <linearGradient id={`${id}-panel`} x1="15%" y1="0%" x2="60%" y2="100%">
      <stop stopColor="#314760" />
      <stop offset="1" stopColor="#17273c" />
    </linearGradient>
    <linearGradient id={`${id}-socket`} x1="0%" y1="0%" x2="0%" y2="100%">
      <stop stopColor="#080f1c" />
      <stop offset="1" stopColor="#26364b" />
    </linearGradient>
    <linearGradient id={`${id}-trim`}>
      <stop stopColor="#548ca2" />
      <stop offset=".16" stopColor="#a1ebee" />
      <stop offset=".72" stopColor="#9bb1cf" />
      <stop offset="1" stopColor="#b3b4e9" />
    </linearGradient>
  </defs>;
}

function GatewayLid({ id }: { id: string }) {
  return <g>
    <path d={LID} fill={paint(id, "lid")} stroke="#b4c4d5" strokeWidth="1.3" />
    <path d="M108 135 194 78Q199 74 211 74L605 86Q614 86 618 88" fill="none"
      stroke="#f6fbff" strokeWidth="2.2" strokeLinecap="round" opacity=".8" />
    <g transform={LID_PLANE}>
      <rect x="45" y="22" width="356" height="74" rx="8" fill="none" stroke="#70869d" strokeOpacity=".56" />
      <rect x="125" y="31" width="222" height="56" rx="8" fill="#7890a9" fillOpacity=".4"
        stroke="#dce7f1" strokeOpacity=".64" />
      {VENTS.map((index) => (
        <rect key={index} x={136 + index * 13} y="39" width="5.5" height="40" rx="2.75"
          fill="#3d5169" fillOpacity=".82" stroke="#71869e" strokeWidth=".7" />
      ))}
      <g transform="translate(66 43)" fill="none" stroke="#536e89" strokeWidth="2.7" strokeLinecap="round" strokeLinejoin="round">
        <path d="M0 0h32v32H0zM8 16h16M17 8l8 8-8 8" />
      </g>
    </g>
    <path d="M110 140 517 152Q529 152 537 147L615 96" fill="none"
      stroke="#eff6fc" strokeOpacity=".82" strokeWidth="2" strokeLinecap="round" />
  </g>;
}

function NetworkSocket({ id, x }: { id: string; x: number }) {
  return <g className="asb-gateway-socket" transform={`translate(${x} 22)`}>
    <rect width="56" height="41" rx="5" fill="#091522" stroke="#8297af" />
    <path d="M6 6h44v21h-8v8H14v-8H6Z" fill={paint(id, "socket")} stroke="#485c73" />
    <path d="M14 7v8m5-8v8m5-8v8m5-8v8m5-8v8m5-8v8" stroke="#d0b77c" strokeWidth="1.6" />
    <path d="M15 27h26" fill="none" stroke="#668d9f" strokeOpacity=".42" />
    <path className="asb-gateway-socket-glow" d="M13 18h30m-26 5h22" fill="none"
      stroke="#a9f3ee" strokeWidth="1.9" strokeLinecap="round" />
  </g>;
}

function GatewayFace({ id }: { id: string }) {
  return <g transform={FRONT_PLANE}>
    <rect x="16" y="79" width="45" height="15" rx="4" fill="#223247" />
    <rect x="359" y="79" width="45" height="15" rx="4" fill="#223247" />
    <rect width="426" height="87" rx="12" fill={paint(id, "frame")} stroke="#73849a" />
    <rect x="10" y="10" width="405" height="65" rx="8" fill={paint(id, "panel")} stroke="#95a8bc" strokeOpacity=".48" />
    <path d="M23 79h379" stroke={paint(id, "trim")} strokeWidth="2" strokeLinecap="round" />
    <g>
      {[29, 98, 167, 236].map((x) => <NetworkSocket key={x} id={id} x={x} />)}
    </g>
    <g transform="translate(312 22)">
      <rect width="56" height="41" rx="4" fill="#8a9aaf" stroke="#b4c2d2" />
      <rect x="5" y="6" width="46" height="28" rx="2" fill="#0c1726" />
      <path d="M9 10h38m-38 18h38" stroke="#4f657f" strokeWidth="2" />
      <path d="M19 32h18v5H19Z" fill="#afbfce" />
    </g>
    <circle cx="391" cy="43" r="10" fill="#141f31" stroke="#788da4" />
    <path d="M391 38v6m-3-4a4.5 4.5 0 1 0 6 0" fill="none"
      stroke="#9eafbf" strokeWidth="1.1" strokeLinecap="round" />
  </g>;
}

export function GatewayArtwork({ active }: { active: boolean }) {
  return <svg className="asb-gateway-art" data-active={active || undefined}
    viewBox="36 15 648 270" width="648" height="270" aria-hidden="true" focusable="false">
    <GatewayChassis />
  </svg>;
}

export function GatewayChassis() {
  const id = useId();
  return <g>
    <ChassisPaints id={id} />
    <ellipse cx="353" cy="231" rx="288" ry="42" fill={paint(id, "shadow")} />
    <path d={SIDE} fill={paint(id, "side")} stroke="#566d86" strokeWidth="1.2" />
    <g transform={SIDE_PLANE}>
      <rect x="20" y="19" width="71" height="48" rx="6" fill="#263b54" stroke="#7c90a7" strokeOpacity=".68" />
      <path d="M31 30h49m-49 9h49m-49 9h49m-49 9h49" stroke="#152639" strokeWidth="3" strokeLinecap="round" />
      <path d="M14 77h84" stroke="#9eaac5" strokeOpacity=".6" strokeWidth="1.6" strokeLinecap="round" />
    </g>
    <GatewayFace id={id} />
    <GatewayLid id={id} />
  </g>;
}
