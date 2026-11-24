import { useId } from "react";
import { ChassisPaints } from "./model-routing/GatewayArtwork";

/* The storage unit shares the gateway's fixed 2.5D body: the front face stays
   frontal, depth runs up and to the right. Its face carries three drive
   drawers and its lid a platter window, so it reads as a disk enclosure
   beside the gateway's network sockets. */
const LID = "M105 132 190 76Q199 70 210 70L607 82Q623 82 623 90Q623 94 615 99L535 151Q529 155 517 155L113 143Q91 142 105 132Z";
const SIDE = "M523 150 614 92Q623 86 623 91V167Q623 176 615 181L535 234Q529 238 522 238Z";
const LID_PLANE = "matrix(1 .03 .82 -.58 102 138)";
const FRONT_PLANE = "matrix(1 .03 0 1 102 138)";
const SIDE_PLANE = "matrix(.82 -.58 0 1 527 151)";
const DRAWERS = [0, 1, 2];
const VENTS = Array.from({ length: 15 }, (_, index) => index);

function paint(id: string, layer: string) {
  return `url(#${id}-${layer})`;
}

function StorageLid({ id }: { id: string }) {
  return <g>
    <path d={LID} fill={paint(id, "lid")} stroke="#b4c4d5" strokeWidth="1.3" />
    <path d="M108 135 194 78Q199 74 211 74L605 86Q614 86 618 88" fill="none"
      stroke="#f6fbff" strokeWidth="2.2" strokeLinecap="round" opacity=".8" />
    <g transform={LID_PLANE}>
      <rect x="45" y="22" width="356" height="74" rx="8" fill="none" stroke="#70869d" strokeOpacity=".56" />
      <g transform="translate(96 59)">
        <circle r="31" fill="#223247" stroke="#7c90a7" strokeOpacity=".7" />
        <circle r="26.5" fill="#0c1726" />
        <circle r="20.5" fill="none" stroke="#2c4562" strokeWidth="5.5" />
        <circle r="12.5" fill="none" stroke="#2c4562" strokeWidth="4" />
        <path d="M-17 -17A24 24 0 0 1 17 -17" fill="none" stroke="#a9f3ee" strokeOpacity=".38" strokeWidth="1.5" strokeLinecap="round" />
        <path d="M37 -25 25 -14 10 -5" fill="none" stroke="#9cacc0" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
        <circle cx="37" cy="-25" r="3.2" fill="#b3c5d8" />
        <circle r="4.2" fill="#b3c5d8" />
        <circle r="1.5" fill="#0c1726" />
      </g>
      {VENTS.map((vent) => (
        <rect key={vent} x={150 + vent * 13} y="39" width="5.5" height="40" rx="2.75"
          fill="#3d5169" fillOpacity=".82" stroke="#71869e" strokeWidth=".7" />
      ))}
    </g>
    <path d="M110 140 517 152Q529 152 537 147L615 96" fill="none"
      stroke="#eff6fc" strokeOpacity=".82" strokeWidth="2" strokeLinecap="round" />
  </g>;
}

function DriveDrawer({ id, index }: { id: string; index: number }) {
  return <g className="asb-storage-drawer" transform={`translate(24 ${16 + index * 18})`}>
    <rect width="238" height="15" rx="3" fill={paint(id, "socket")} stroke="#485c73" />
    <rect x="2.5" y="2.5" width="233" height="10" rx="2" fill="#8a9aaf" stroke="#b4c2d2" />
    <path d="M7 4.4h224" stroke="#e6eef6" strokeOpacity=".5" strokeWidth=".9" strokeLinecap="round" />
    <path d="M14 6.6h46m-46 3h30" stroke="#4f657f" strokeWidth="1.1" strokeLinecap="round" />
    <path d="M198 5v5m4-5v5m4-5v5" stroke="#5b7089" strokeWidth="1.1" strokeLinecap="round" />
    <circle className="asb-storage-led-glow" cx="224" cy="7.5" r="5.5" fill="currentColor"
      style={{ animationDelay: `calc(var(--asb-motion-ambient) * ${-1.2 * index})` }} />
    <circle className="asb-storage-led" cx="224" cy="7.5" r="1.9" fill="currentColor" />
  </g>;
}

function StorageFace({ id }: { id: string }) {
  return <g transform={FRONT_PLANE}>
    <rect x="16" y="79" width="45" height="15" rx="4" fill="#223247" />
    <rect x="359" y="79" width="45" height="15" rx="4" fill="#223247" />
    <rect width="426" height="87" rx="12" fill={paint(id, "frame")} stroke="#73849a" />
    <rect x="10" y="10" width="405" height="65" rx="8" fill={paint(id, "panel")} stroke="#95a8bc" strokeOpacity=".48" />
    <path d="M23 79h379" stroke={paint(id, "trim")} strokeWidth="2" strokeLinecap="round" />
    {DRAWERS.map((index) => <DriveDrawer key={index} id={id} index={index} />)}
    <g transform="translate(282 16)">
      <rect width="74" height="51" rx="5" fill="#263b54" stroke="#7c90a7" strokeOpacity=".68" />
      <path d="M9 9h56m-56 8h56m-56 8h56m-56 8h56m-56 8h56" stroke="#152639" strokeWidth="3" strokeLinecap="round" />
    </g>
    <circle cx="391" cy="43" r="10" fill="#141f31" stroke="#788da4" />
    <path d="M391 38v6m-3-4a4.5 4.5 0 1 0 6 0" fill="none"
      stroke="#9eafbf" strokeWidth="1.1" strokeLinecap="round" />
  </g>;
}

/** Decorative only: every capacity fact stays in the text beneath it. */
export function StorageArtwork({ active }: { active: boolean }) {
  const id = useId();
  return <svg className="asb-storage-art" data-active={active || undefined}
    viewBox="36 15 648 270" width="648" height="270" aria-hidden="true" focusable="false">
    <defs><ChassisPaints id={id} /></defs>
    <ellipse cx="353" cy="231" rx="288" ry="42" fill={paint(id, "shadow")} />
    <path d={SIDE} fill={paint(id, "side")} stroke="#566d86" strokeWidth="1.2" />
    <g transform={SIDE_PLANE}>
      <rect x="20" y="19" width="71" height="48" rx="6" fill="#263b54" stroke="#7c90a7" strokeOpacity=".68" />
      <path d="M31 30h49m-49 9h49m-49 9h49m-49 9h49" stroke="#152639" strokeWidth="3" strokeLinecap="round" />
      <path d="M14 77h84" stroke="#9eaac5" strokeOpacity=".6" strokeWidth="1.6" strokeLinecap="round" />
    </g>
    <StorageFace id={id} />
    <StorageLid id={id} />
  </svg>;
}
