import { useId } from "react";
import { ChassisPaints } from "./model-routing/GatewayArtwork";

export type BackupArchiveKind = "local" | "cloud" | "cleanup";

/* One archive unit in the gateway's fixed 2.5D projection: the front face
   stays frontal, depth runs up and to the right. Each backup tab shows the
   same material in a different state — stored trays, an encrypted uplink,
   and a tray boxed for archiving. */
const FRONT_PLANE = "matrix(1 .03 0 1 78 84)";
const LID_PLANE = "matrix(1 .03 .82 -.58 78 84)";
const SIDE_PLANE = "matrix(.82 -.58 0 1 342 91.9)";
const TRAYS = [0, 1, 2, 3];
const VENTS = Array.from({ length: 7 }, (_, index) => index);

function paint(id: string, layer: string) {
  return `url(#${id}-${layer})`;
}

function ArchiveTray({ id, x, index }: { id: string; x: number; index: number }) {
  return <g transform={`translate(${x} 17)`}>
    <rect width="48" height="81" rx="5" fill={paint(id, "socket")} stroke="#8297af" />
    <rect x="4" y="4" width="40" height="73" rx="3" fill={paint(id, "lid")} stroke="#dce7f1" strokeOpacity=".64" />
    <rect x="13" y="9" width="22" height="6" rx="3" fill="#0c1726" />
    <path d="M11 24h26m-26 5h26m-26 5h26" stroke="#536e89" strokeOpacity=".55" strokeWidth="1.4" strokeLinecap="round" />
    <rect x="9" y="57" width="16" height="7" rx="2" fill="#dce7f1" />
    <circle className="asb-backup-tray-glow" cx="36.5" cy="60.5" r="8" fill="currentColor"
      style={{ animationDelay: `calc(var(--asb-motion-ambient) * ${-0.9 * index})` }} />
    <circle className="asb-backup-tray-light" cx="36.5" cy="60.5" r="2.8" fill="currentColor" />
  </g>;
}

function ArchiveFace({ id }: { id: string }) {
  return <g transform={FRONT_PLANE}>
    <path d="M16 124h34v7q0 4-4 4H20q-4 0-4-4Zm198 0h34v7q0 4-4 4h-26q-4 0-4-4Z" fill="#223247" />
    <rect width="264" height="128" rx="12" fill={paint(id, "frame")} stroke="#73849a" />
    <rect x="9" y="9" width="246" height="110" rx="8" fill={paint(id, "panel")} stroke="#95a8bc" strokeOpacity=".48" />
    {TRAYS.map((tray) => <ArchiveTray key={tray} id={id} index={tray} x={22.5 + tray * 57} />)}
    <path d="M22 112h44m10 0h18" stroke="#668d9f" strokeOpacity=".5" strokeWidth="2" strokeLinecap="round" />
    <circle cx="240" cy="112" r="5.5" fill="#141f31" stroke="#788da4" />
    <path d="M240 109.4v3.2m-2.2-2a3 3 0 1 0 4.4 0" fill="none" stroke="#9eafbf" strokeWidth="1" strokeLinecap="round" />
  </g>;
}

function ArchiveLid({ id }: { id: string }) {
  return <g transform={LID_PLANE}>
    <path d="M0 0h264v62H0Z" fill={paint(id, "lid")} stroke="#b4c4d5" strokeWidth="1.3" />
    <path d="M6 4h252" stroke="#f6fbff" strokeWidth="2.2" strokeLinecap="round" opacity=".8" />
    <rect x="22" y="10" width="88" height="42" rx="7" fill="none" stroke="#70869d" strokeOpacity=".56" />
    <g transform="translate(55 20)" fill="none" stroke="#536e89" strokeWidth="2.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
    </g>
    {VENTS.map((vent) => <rect key={vent} x={170 + vent * 11.5} y="10" width="5.5" height="42" rx="2.75"
      fill="#3d5169" fillOpacity=".82" stroke="#71869e" strokeWidth=".7" />)}
  </g>;
}

function ArchiveSide({ id }: { id: string }) {
  return <g transform={SIDE_PLANE}>
    <path d="M0 0h62v128H0Z" fill={paint(id, "side")} stroke="#566d86" strokeWidth="1.2" />
    <rect x="14" y="20" width="36" height="46" rx="6" fill="#263b54" stroke="#7c90a7" strokeOpacity=".68" />
    <path d="M23 31h18m-18 9h18m-18 9h18m-18 9h18" stroke="#152639" strokeWidth="3" strokeLinecap="round" />
    <path d="M10 78h42" stroke="#9eaac5" strokeOpacity=".6" strokeWidth="1.6" strokeLinecap="round" />
  </g>;
}

function ArchiveUnit({ id }: { id: string }) {
  return <>
    <ellipse cx="235" cy="214" rx="172" ry="26" fill={paint(id, "shadow")} />
    <ArchiveSide id={id} />
    <ArchiveFace id={id} />
    <ArchiveLid id={id} />
  </>;
}

/** A cloud with a lock above the unit, linked through one port on the lid. */
function CloudUplink({ id }: { id: string }) {
  return <>
    <linearGradient id={`${id}-cloud`} x1="0" y1="-32" x2="0" y2="34" gradientUnits="userSpaceOnUse">
      <stop stopColor="#ffffff" />
      <stop offset="1" stopColor="#b9c9da" />
    </linearGradient>
    <g fill="#8da2bc">
      <rect x="152.2" y="0.2" width="107.6" height="35.1" rx="17.3" />
      <circle cx="178.5" cy="9" r="22.5" /><circle cx="206" cy="-3.5" r="30" /><circle cx="236" cy="9" r="21.3" />
    </g>
    <g fill={paint(id, "cloud")}>
      <rect x="153.5" y="1.5" width="105" height="32.5" rx="16" />
      <circle cx="178.5" cy="9" r="21.25" /><circle cx="206" cy="-3.5" r="28.75" /><circle cx="236" cy="9" r="20" />
    </g>
    <path d="M199.6 5V.6a6.6 6.6 0 0 1 13.2 0V5" fill="none" stroke="#2b4360" strokeWidth="3" strokeLinecap="round" />
    <rect x="195.2" y="4.8" width="21.6" height="16.8" rx="3.6" fill="#2b4360" />
    <circle cx="206" cy="12" r="2" fill="#b9c9da" />
    <path d="M206 38v27" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeDasharray="3 6" />
    <ellipse cx="206" cy="70" rx="8" ry="3.6" fill="#17273c" stroke="currentColor" strokeWidth="1.4" />
  </>;
}

/** One tray leaving the unit for a sealed archive box. */
function ArchiveHandoff({ id }: { id: string }) {
  return <>
    <ellipse cx="73" cy="196" rx="42" ry="11" fill={paint(id, "shadow")} />
    <ellipse cx="308" cy="195" rx="92" ry="16" fill={paint(id, "shadow")} />
    <g transform="matrix(.82 -.58 0 1 106 88)"><path d="M0 0h16v110H0Z" fill={paint(id, "side")} stroke="#566d86" /></g>
    <g transform="matrix(1 .03 .82 -.58 40 86)"><path d="M0 0h66v16H0Z" fill={paint(id, "lid")} stroke="#b4c4d5" /></g>
    <g transform="matrix(1 .03 0 1 40 86)">
      <rect width="66" height="110" rx="6" fill={paint(id, "socket")} stroke="#8297af" />
      <rect x="5" y="5" width="56" height="100" rx="3.5" fill={paint(id, "lid")} stroke="#dce7f1" strokeOpacity=".64" />
      <rect x="18" y="11" width="30" height="8" rx="4" fill="#0c1726" />
      <path d="M15 30h36m-36 7h36m-36 7h36" stroke="#536e89" strokeOpacity=".55" strokeWidth="1.6" strokeLinecap="round" />
      <rect x="12" y="78" width="22" height="9" rx="2" fill="#dce7f1" />
      <circle className="asb-backup-tray-light" cx="50" cy="82.5" r="3.6" fill="currentColor" />
    </g>
    <path d="M128 142h70m0 0-9-7m9 7-9 7" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" />
    <g transform="matrix(.82 -.58 0 1 348 103.5)"><path d="M0 0h44v94H0Z" fill={paint(id, "side")} stroke="#566d86" /></g>
    <g transform="matrix(1 .03 .82 -.58 232 100)">
      <path d="M0 0h116v44H0Z" fill={paint(id, "lid")} stroke="#b4c4d5" />
      <path d="M58 0v44" stroke="#9cacc0" strokeWidth=".9" />
      <rect x="52" y="0" width="12" height="44" fill="#7890a9" fillOpacity=".55" />
    </g>
    <g transform="matrix(1 .03 0 1 232 100)">
      <rect width="116" height="94" rx="9" fill={paint(id, "lid")} stroke="#8da2bc" />
      <path d="M0 26h116" stroke="#8da2bc" strokeWidth=".9" strokeOpacity=".7" />
      <rect x="52" y="0" width="12" height="94" fill="#7890a9" fillOpacity=".55" />
      <rect x="80" y="36" width="22" height="6" rx="3" fill="#3d5169" />
      <rect x="14" y="42" width="30" height="22" rx="3" fill="#f4f8fc" stroke="#9cacc0" strokeWidth=".8" />
      <path d="M19 50h20m-20 6h14" stroke="#536e89" strokeWidth="1.6" strokeLinecap="round" />
    </g>
  </>;
}

const VIEW_BOX: Record<BackupArchiveKind, string> = {
  local: "0 0 470 246",
  cloud: "0 -52 470 298",
  cleanup: "0 40 424 190",
};

/** Decorative only: every backup fact stays in the text beside it. */
export function BackupArchiveArt({ kind, active = false }: { kind: BackupArchiveKind; active?: boolean }) {
  const id = useId();
  return <svg className="asb-backup-archive-art" data-kind={kind} data-active={active || undefined}
    viewBox={VIEW_BOX[kind]} aria-hidden="true" focusable="false">
    <defs><ChassisPaints id={id} /></defs>
    {kind === "cleanup" ? <ArchiveHandoff id={id} /> : <>
      {kind === "cloud" && <CloudUplink id={id} />}
      <ArchiveUnit id={id} />
    </>}
  </svg>;
}
