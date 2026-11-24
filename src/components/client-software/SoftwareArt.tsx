import { useId } from "react";
import type { SoftwareForm } from "../../api/client-software";
import { DevicePaints, GatewayDeviceArt, devicePaint } from "../GatewayDeviceArt";

/* One device per installation form, in the gateway topology's fixed view and
 * metal: the front face stays frontal, depth runs up and to the right. The
 * command line tool reuses the topology's client terminal, so a client reads
 * as the same device on both pages. Decorative only — every installation fact
 * stays in the card's text. */

function Monitor({ id }: { id: string }) {
  return <g>
    <path d="M73 76h14v17l13 7H60l13-7Z" fill={devicePaint(id, "edge")} />
    <path d="M60 100h40l14 6H46Z" fill={devicePaint(id, "alloy")} stroke="#8196b0" />
    <path d="M46 106h68v3H48q-2 0-2-3Z" fill="#354862" />
    <rect x="52" y="12" width="88" height="54" rx="6" fill={devicePaint(id, "edge")} stroke="#607896" />
    <path d="M39 22 55 13q3-2 7-2h70q8 0 8 7l-17 9H45q-6 0-6-5Z" fill={devicePaint(id, "alloy")} stroke="#9cacc0" />
    <rect x="36" y="22" width="88" height="54" rx="6" fill={devicePaint(id, "alloy")} stroke="#889db5" />
    <rect x="40" y="26" width="80" height="46" rx="3.5" fill={devicePaint(id, "recess")} stroke="#546d88" />
    <rect x="44" y="30" width="72" height="38" rx="2" fill={devicePaint(id, "screen")} />
    <rect x="62" y="38" width="36" height="24" rx="3" fill="#141f31" stroke="currentColor" strokeWidth="1.6" />
    <path d="M62 45h36" stroke="currentColor" strokeWidth="1.6" />
    <path d="M68 51h14m-14 5h22" stroke="#a0b7d3" strokeOpacity=".55" strokeWidth="1.4" strokeLinecap="round" />
  </g>;
}

function Editor({ id }: { id: string }) {
  return <g>
    <rect x="38" y="16" width="116" height="62" rx="7" fill={devicePaint(id, "edge")} stroke="#607896" />
    <path d="M25 26 41 17q3-2 7-2h99q7 0 7 7l-16 9H31q-6 0-6-5Z" fill={devicePaint(id, "alloy")} stroke="#9cacc0" />
    <rect x="22" y="26" width="116" height="62" rx="7" fill={devicePaint(id, "alloy")} stroke="#889db5" />
    <rect x="26" y="30" width="108" height="54" rx="4" fill={devicePaint(id, "recess")} stroke="#546d88" />
    <path d="M26 34a4 4 0 0 1 4-4h11v54H30a4 4 0 0 1-4-4Z" fill="#1a2c42" />
    {[37, 48, 59, 70].map((y) => <rect key={y} x="30.5" y={y} width="6" height="6" rx="1.5"
      fill={y === 48 ? "currentColor" : "#8fa6c0"} fillOpacity={y === 48 ? 1 : .5} />)}
    <path d="M45 41h87" stroke="#a0b7d3" strokeOpacity=".26" />
    <rect x="47" y="33" width="24" height="8" rx="2" fill="#2a4260" />
    <path d="M48 48h34m-34 6h52m-34 6h26m-8 6h42m-52 6h38m4 6h20" stroke="#afc4dd" strokeOpacity=".5"
      strokeWidth="2.2" strokeLinecap="round" />
    <path d="M48 48h10" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
    <rect x="98" y="46" width="36" height="38" rx="3" fill="#1a2c42" stroke="currentColor" strokeWidth="1.3" />
    <path d="M112 60v-4m8 4v-4" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
    <path d="M110 60h12v6a6 6 0 0 1-12 0Z" fill="currentColor" />
    <path d="M116 72v4l-5 4h-5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </g>;
}

function Chip({ id }: { id: string }) {
  return <g>
    <path d="M30 84 46 74h100l-14 10Z" fill={devicePaint(id, "alloy")} stroke="#9cacc0" />
    <rect x="28" y="84" width="104" height="10" rx="3" fill={devicePaint(id, "edge")} stroke="#607896" />
    <path d="M108 79h16l8-6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    {[40, 56, 72].map((y) => <g key={y}>
      <rect x="48" y={y} width="8" height="4" rx="1" fill="#b9c7d6" stroke="#7e91a9" strokeWidth=".5" />
      <rect x="108" y={y} width="8" height="4" rx="1" fill="#b9c7d6" stroke="#7e91a9" strokeWidth=".5" />
    </g>)}
    {[62, 80, 98].map((x) => <rect key={x} x={x} y="79" width="4" height="7" rx="1" fill="#b9c7d6" stroke="#7e91a9" strokeWidth=".5" />)}
    <path d="M56 32 70 22h52L108 32Z" fill={devicePaint(id, "alloy")} stroke="#9cacc0" />
    <path d="M108 32l14-10v52l-14 10Z" fill="#536a88" stroke="#607896" strokeWidth=".8" />
    <rect x="56" y="32" width="52" height="52" rx="3" fill="#263a53" stroke="#7e91a9" />
    <rect x="66" y="42" width="32" height="32" rx="2" fill={devicePaint(id, "recess")} stroke="#8fa6c0" strokeOpacity=".6" />
    <path d="M82 49l9 9-9 9-9-9Z" fill="currentColor" />
  </g>;
}

/** The installation's device in the topology metal; the client tone is the accent. */
export function SoftwareArt({ form }: { form: SoftwareForm }) {
  const id = useId();
  if (form === "cli") return <GatewayDeviceArt kind="client" connected={false} />;
  return <svg className={`asb-gateway-device is-${form}`} viewBox="0 0 160 116" fill="none" aria-hidden="true" focusable="false">
    <DevicePaints id={id} />
    <ellipse cx="80" cy="103" rx="65" ry="12" fill={devicePaint(id, "shadow")} />
    {form === "desktop" ? <Monitor id={id} /> : form === "extension" ? <Editor id={id} /> : <Chip id={id} />}
  </svg>;
}
