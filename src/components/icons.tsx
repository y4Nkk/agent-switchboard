/** Inline functional icons (stroke = currentColor). No icon library: each
 * glyph is a hand-drawn inline SVG so the bundle stays dependency-free. */
import type { ReactNode } from "react";

interface IconProps {
  size?: number;
}

function svg(path: ReactNode, size: number) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {path}
    </svg>
  );
}

export function PreviewIcon({ size = 18 }: IconProps) {
  return svg(
    <>
      <path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6-10-6-10-6Z" />
      <circle cx="12" cy="12" r="3" />
    </>,
    size,
  );
}

/** The preview button's open state: the eye closes, clicking retracts. */
export function EyeOffIcon({ size = 18 }: IconProps) {
  return svg(
    <>
      <path d="M3 3l18 18" />
      <path d="M10.6 5.2C11 5.1 11.5 5 12 5c6.5 0 10 6 10 6a17.6 17.6 0 0 1-3.1 3.8M6.6 6.6C3.8 8.4 2 11 2 11s3.5 6 10 6c1.3 0 2.5-.2 3.6-.7" />
      <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
    </>,
    size,
  );
}

export function EditIcon({ size = 18 }: IconProps) {
  return svg(
    <>
      <path d="M16.7 3.3a2 2 0 0 1 2.8 0l1.2 1.2a2 2 0 0 1 0 2.8L8 19.6 3 21l1.4-5Z" />
      <path d="m14.5 5.5 4 4" />
    </>,
    size,
  );
}

export function TrashIcon({ size = 18 }: IconProps) {
  return svg(
    <>
      <path d="M3 6h18" />
      <path d="M8 6V4h8v2" />
      <path d="m19 6-1 15H6L5 6" />
      <path d="M10 11v6M14 11v6" />
    </>,
    size,
  );
}

export function PlusIcon({ size = 20 }: IconProps) {
  return svg(
    <>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </>,
    size,
  );
}

export function MinimizeIcon({ size = 16 }: IconProps) {
  return svg(<path d="M5 12h14" />, size);
}

export function MaximizeIcon({ size = 16 }: IconProps) {
  return svg(<rect x="6" y="6" width="12" height="12" rx="1.5" />, size);
}

export function RestoreIcon({ size = 16 }: IconProps) {
  return svg(
    <>
      <path d="M8.5 4.5H6A1.5 1.5 0 0 0 4.5 6v2.5" />
      <rect x="8.5" y="8.5" width="11" height="11" rx="1.5" />
    </>,
    size,
  );
}

export function CloseIcon({ size = 16 }: IconProps) {
  return svg(
    <>
      <path d="m6 6 12 12" />
      <path d="m18 6-12 12" />
    </>,
    size,
  );
}

export function PinIcon({ size = 16 }: IconProps) {
  return svg(
    <>
      <path d="M12 17v5" />
      <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1z" />
    </>,
    size,
  );
}

export function GripIcon({ size = 18 }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="currentColor"
      aria-hidden="true"
    >
      <circle cx="9" cy="6" r="1.6" />
      <circle cx="9" cy="12" r="1.6" />
      <circle cx="9" cy="18" r="1.6" />
      <circle cx="15" cy="6" r="1.6" />
      <circle cx="15" cy="12" r="1.6" />
      <circle cx="15" cy="18" r="1.6" />
    </svg>
  );
}

export function PlayIcon({ size = 16 }: IconProps) {
  return svg(<path d="M8 5.3v13.4L19 12Z" />, size);
}

/** Update-available glyph: a download arrow landing on the release tray. */
export function UpdateIcon({ size = 16 }: IconProps) {
  return svg(
    <>
      <path d="M12 4v11" />
      <path d="m7.5 11.5 4.5 4.5 4.5-4.5" />
      <path d="M5 20h14" />
    </>,
    size,
  );
}

/** Account-usage readout: three measured bars above a baseline. */
export function UsageIcon({ size = 16 }: IconProps) {
  return svg(
    <>
      <path d="M4 20h16" />
      <path d="M6.5 16v-4" />
      <path d="M12 16V7" />
      <path d="M17.5 16v-7" />
    </>,
    size,
  );
}

/** Endpoint reachability: radio arcs converge on the service address. */
export function ConnectivityIcon({ size = 16 }: IconProps) {
  return svg(
    <>
      <path d="M5.5 10.2a9.2 9.2 0 0 1 13 0" />
      <path d="M8.3 13a5.2 5.2 0 0 1 7.4 0" />
      <path d="M10.6 15.8a2 2 0 0 1 2.8 0" />
      <circle cx="12" cy="18.4" r=".8" fill="currentColor" stroke="none" />
    </>,
    size,
  );
}
