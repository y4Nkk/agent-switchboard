import { useId } from "react";
import { GatewayChassis } from "./model-routing/GatewayArtwork";

type DeviceKind = "client" | "hub" | "provider";

function paint(id: string, layer: string) {
  return `url(#${id}-${layer})`;
}

/** Device ports mark the visual endpoints for the live topology. */
export function GatewayDeviceArt({ kind, connected = true }: { kind: DeviceKind; connected?: boolean }) {
  const id = useId();
  return (
    <svg className={`asb-gateway-device is-${kind}`} viewBox={kind === "hub" ? "0 24 160 66" : "0 0 160 116"}
      fill="none" aria-hidden="true" focusable="false">
      {kind === "hub" ? <>
        <g transform="translate(-12 15) scale(.255)"><GatewayChassis /></g>
        {connected && <>
          <NetworkJack x={12} y={61} direction="input" />
          <NetworkJack x={148} y={55} direction="output" />
        </>}
      </> : <>
        <DevicePaints id={id} />
        <ellipse cx="80" cy="103" rx="65" ry="12" fill={paint(id, "shadow")} />
        {kind === "client" ? <Terminal id={id} /> : <Server id={id} />}
        {connected && (kind === "client"
          ? <NetworkJack x={140} y={63} direction="output" />
          : <NetworkJack x={21} y={63} direction="input" />)}
      </>}
    </svg>
  );
}

function DevicePaints({ id }: { id: string }) {
  return <defs>
    <radialGradient id={`${id}-shadow`}>
      <stop stopColor="#263a57" stopOpacity=".32" />
      <stop offset="1" stopColor="#263a57" stopOpacity="0" />
    </radialGradient>
    <linearGradient id={`${id}-alloy`} x2="95%" y2="100%">
      <stop stopColor="#f1f6fb" />
      <stop offset=".48" stopColor="#c3d1e0" />
      <stop offset="1" stopColor="#7e93ad" />
    </linearGradient>
    <linearGradient id={`${id}-edge`} x2="100%" y2="90%">
      <stop stopColor="#8da2bc" />
      <stop offset="1" stopColor="#536a88" />
    </linearGradient>
    <linearGradient id={`${id}-recess`} x2="72%" y2="100%">
      <stop stopColor="#344d69" />
      <stop offset="1" stopColor="#152439" />
    </linearGradient>
    <linearGradient id={`${id}-screen`} x2="26%" y2="100%">
      <stop stopColor="#192f48" />
      <stop offset=".68" stopColor="#213754" />
      <stop offset="1" className="asb-gateway-device-screen-tint" />
    </linearGradient>
  </defs>;
}

function Terminal({ id }: { id: string }) {
  return <g>
    <path d="M68 79h21v14l15 6v5H49v-5l19-6Z" fill={paint(id, "edge")} />
    <path d="M68 93h21l15 6H49Z" fill={paint(id, "alloy")} stroke="#8196b0" />
    <path d="M22 27 36 17Q39 15 44 15h83q10 0 10 11v48l-12 11Z" fill={paint(id, "edge")} />
    <path d="M25 27 37 18q3-2 7-2h83q5 0 7 3l-13 10Z" fill={paint(id, "alloy")} />
    <rect x="20" y="24" width="108" height="62" rx="9" fill={paint(id, "alloy")} stroke="#889db5" />
    <rect x="24" y="28" width="100" height="54" rx="6" fill={paint(id, "recess")} stroke="#546d88" />
    <rect x="29" y="33" width="90" height="44" rx="4" fill={paint(id, "screen")} />
    <path d="M35 37h77" stroke="#e2effc" strokeOpacity=".18" strokeLinecap="round" />
    <path className="asb-gateway-device-accent" d="m40 44 8 6-8 6m14 0h16"
      strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
    <path d="M41 64h33m-33 5h49" stroke="#a0b7d3" strokeOpacity=".55" strokeWidth="1.4" strokeLinecap="round" />
  </g>;
}

function Server({ id }: { id: string }) {
  return <g>
    <path d="M30 95h16v10H32q-2 0-2-2Zm64 0h16v7q0 2-2 2H94Z" fill="#354862" />
    <path d="M113 30l23-15q5 1 5 7v59q0 5-4 8l-23 15Z" fill={paint(id, "edge")} stroke="#607896" />
    <path d="M24 30 45 15q4-3 9-3h76q8 0 11 6l-24 15Z" fill={paint(id, "alloy")} stroke="#9cacc0" />
    <path d="m59 18 51 0m-59 6h51" stroke="#8196ad" strokeWidth="1.4" strokeLinecap="round" opacity=".72" />
    <path d="m127 41 8-6m-8 14 8-6m-8 14 8-6m-8 14 8-6" stroke="#3d5573" strokeWidth="2.2" strokeLinecap="round" />
    <rect x="24" y="28" width="96" height="74" rx="7" fill={paint(id, "alloy")} stroke="#889db5" />
    <rect x="28" y="32" width="88" height="66" rx="5" fill={paint(id, "recess")} />
    {[36, 56, 76].map((y) => <g key={y} transform={`translate(32 ${y})`}>
      <rect width="80" height="17" rx="3" fill={paint(id, "edge")} stroke="#304760" />
      <path d="M6 4h68" stroke="#dce8f3" strokeOpacity=".34" />
      <rect x="7" y="7" width="44" height="6" rx="2" fill="#273d57" />
      <path d="M12 9h32m-29 2h27" stroke="#afc4dd" strokeOpacity=".45" strokeLinecap="round" />
      <path d="M56 10h8" stroke="currentColor" strokeWidth="2.1" strokeLinecap="round" />
      <rect x="70" y="8" width="4" height="4" rx="1" fill="#b3c5d8" />
    </g>)}
  </g>;
}

function NetworkJack({ x, y, direction }: { x: number; y: number; direction: "input" | "output" }) {
  return <g>
    <rect x={direction === "input" ? x : x - 7} y={y - 4} width="7" height="8" rx="2"
      fill="#263a53" stroke="#7e91a9" />
    <circle data-gateway-port={direction} cx={x} cy={y} r="3.25"
      className="asb-gateway-device-jack" stroke="currentColor" strokeWidth="1.4" />
  </g>;
}
