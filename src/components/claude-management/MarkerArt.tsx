import { useId } from "react";
import { DevicePaints, devicePaint } from "../GatewayDeviceArt";

export type MarkerState = "written" | "absent" | "pending";

/* One marker as a toggle on a small panel, in the gateway topology's fixed
 * view and metal: the lever sits up once the marker is written and down when
 * it is not; the slot is only outlined while the target file does not exist.
 * Decorative only — the state stays in the card's status pill and text. */
export function MarkerArt({ state }: { state: MarkerState }) {
  const id = useId();
  return <svg className="asb-marker-art" data-state={state} viewBox="0 0 160 96" fill="none" aria-hidden="true" focusable="false">
    <DevicePaints id={id} />
    <ellipse cx="80" cy="88" rx="58" ry="8" fill={devicePaint(id, "shadow")} />
    <rect x="32" y="16" width="100" height="54" rx="8" fill={devicePaint(id, "edge")} stroke="#607896" />
    <path d="M20 26 36 17q3-2 7-2h81q8 0 8 7l-16 9H26q-6 0-6-5Z" fill={devicePaint(id, "alloy")} stroke="#9cacc0" />
    <rect x="18" y="26" width="100" height="54" rx="8" fill={devicePaint(id, "alloy")} stroke="#889db5" />
    <rect x="24" y="32" width="88" height="42" rx="5" fill={devicePaint(id, "recess")} stroke="#546d88" />
    <path d="M33 46h30m-30 7h20m-20 7h26" stroke="#a0b7d3" strokeOpacity=".45" strokeWidth="1.6" strokeLinecap="round" />
    <circle cx="99" cy="42" r="3.6" fill={state === "written" ? "currentColor" : "#1b2a3d"}
      stroke={state === "written" ? "#e2effc" : "#4f657f"} strokeWidth="1" />
    <rect className="asb-marker-slot" x="94" y="50" width="10" height="18" rx="5" fill="#080f1c" stroke="#485c73" />
    {state !== "pending" && <rect x="95" y={state === "written" ? 44 : 62} width="8" height="12" rx="4"
      fill={devicePaint(id, "alloy")} stroke="#dce8f3" strokeWidth=".8" />}
  </svg>;
}
