import { useId, type ReactNode } from "react";
import type { InterfaceScale, ThemePreference } from "../../api/client";

const PREVIEW_SIZE = { width: 160, height: 104 } as const;

// Preview colors describe the pictured theme, independently of the host theme.
export const LIGHT = {
  surface: "#fafafa", rail: "#eceef1", stroke: "rgba(20,22,27,0.18)",
  text: "#14161b", muted: "rgba(20,22,27,0.56)", faint: "rgba(20,22,27,0.24)",
  accent: "#0a84ff", danger: "#d70015", warning: "#b25000",
  sidebarStart: "var(--asb-sidebar-light-start)",
  sidebarEnd: "var(--asb-sidebar-light-end)",
  sidebarIcon: "var(--asb-sidebar-light-icon)",
};

export const DARK: typeof LIGHT = {
  surface: "#0d2133", rail: "#111e2b", stroke: "rgba(237,240,247,0.24)",
  text: "#edf0f7", muted: "rgba(237,240,247,0.6)", faint: "rgba(237,240,247,0.28)",
  accent: "#0a84ff", danger: "#ff453a", warning: "#ff9f0a",
  sidebarStart: "var(--asb-sidebar-dark-start)",
  sidebarEnd: "var(--asb-sidebar-dark-end)",
  sidebarIcon: "var(--asb-sidebar-dark-icon)",
};

export function PreviewCanvas({ children }: { children: ReactNode }) {
  const { width, height } = PREVIEW_SIZE;
  return <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}
    aria-hidden="true" focusable="false">{children}</svg>;
}

export function WindowFrame({ palette, zoom = 1, active = 0, children }: {
  palette: typeof LIGHT;
  zoom?: number;
  active?: number | null;
  children: ReactNode;
}) {
  const selectedGradient = useId();
  return (
    <>
      <defs>
        <linearGradient id={selectedGradient} x1="0%" y1="0%" x2="100%" y2="0%">
          <stop stopColor={palette.sidebarStart} />
          <stop offset="1" stopColor={palette.sidebarEnd} />
        </linearGradient>
      </defs>
      <rect x="4" y="4" width="152" height="96" rx="10" fill={palette.rail} stroke={palette.stroke} />
      <rect x="12" y="10" width="6" height="6" rx="2" fill={palette.accent} />
      <rect x="24" y="12" width="32" height="3" rx="1.5" fill={palette.muted} />
      <path d="M120 14h5 M134 11h5v5h-5z M146 11l5 5m0-5-5 5"
        fill="none" stroke={palette.muted} strokeLinecap="round" />
      <rect x="30" y="22" width="122" height="74" rx="8" fill={palette.surface} />
      {[32, 48, 64].map((cy, index) => (
        <g key={cy}>
          {index === active && <rect x="8" y={cy - 7} width="14" height="14" rx="5"
            fill={`url(#${selectedGradient})`} />}
          <circle cx="15" cy={cy} r={2.5 * zoom} fill={index === active ? palette.sidebarIcon : palette.muted} />
        </g>
      ))}
      <circle cx="15" cy="86" r="2.5" fill="none" stroke={palette.muted} />
      {children}
    </>
  );
}

export function WindowContent({ palette, zoom = 1 }: { palette: typeof LIGHT; zoom?: number }) {
  return (
    <g transform={`translate(40 30) scale(${zoom})`}>
      <text x="0" y="13" fill={palette.text} fontFamily="var(--asb-font-ui)" fontSize="15" fontWeight="600">Aa</text>
      <rect x="28" y="3" width="40" height="4" rx="2" fill={palette.text} />
      <rect x="28" y="11" width="28" height="3" rx="1.5" fill={palette.muted} />
      <rect y="22" width="82" height="12" rx="4" fill={palette.rail} />
      <circle cx="7" cy="28" r="2" fill={palette.accent} />
      <rect x="15" y="26.5" width="38" height="3" rx="1.5" fill={palette.muted} />
      <rect x="67" y="25" width="9" height="6" rx="3" fill={palette.faint} />
      <rect y="40" width="30" height="9" rx="4.5" fill={palette.accent} />
      <rect x="36" y="40" width="26" height="9" rx="4.5" fill="none" stroke={palette.stroke} />
    </g>
  );
}

// Only theme previews split the scene; scale previews keep the same frame and
// reserve enough room for the complete content at 125%.
export function WindowScene({ theme, scale = 100 }: {
  theme: ThemePreference;
  scale?: InterfaceScale;
}) {
  const zoom = scale / 100;
  const id = useId();
  const left = `${id}-light`;
  const right = `${id}-dark`;
  const palette = theme === "dark" ? DARK : LIGHT;
  return (
    <PreviewCanvas>
      {theme === "system" ? (
        <>
          <defs>
            <clipPath id={left}><rect width={PREVIEW_SIZE.width / 2} height={PREVIEW_SIZE.height} /></clipPath>
            <clipPath id={right}><rect x={PREVIEW_SIZE.width / 2} width={PREVIEW_SIZE.width / 2} height={PREVIEW_SIZE.height} /></clipPath>
          </defs>
          <g clipPath={`url(#${left})`}>
            <WindowFrame palette={LIGHT} zoom={zoom}><WindowContent palette={LIGHT} zoom={zoom} /></WindowFrame>
          </g>
          <g clipPath={`url(#${right})`}>
            <WindowFrame palette={DARK} zoom={zoom}><WindowContent palette={DARK} zoom={zoom} /></WindowFrame>
          </g>
        </>
      ) : (
        <WindowFrame palette={palette} zoom={zoom}><WindowContent palette={palette} zoom={zoom} /></WindowFrame>
      )}
    </PreviewCanvas>
  );
}
