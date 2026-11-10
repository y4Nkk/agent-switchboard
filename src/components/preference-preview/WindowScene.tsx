import { useId, type ReactNode } from "react";
import type { InterfaceScale, ThemePreference } from "../../api/client";
import appIcon from "../../assets/app-icon.svg";

const PREVIEW_SIZE = { width: 400, height: 248 } as const;

// Preview colors describe the pictured theme, independently of the host theme.
export const LIGHT = {
  surface: "#ffffff", rail: "#f0f1f4", stroke: "rgba(20,22,27,0.12)",
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

export function AppMark({ x, y, size }: { x: number; y: number; size: number }) {
  return <image href={appIcon} x={x} y={y} width={size} height={size} />;
}

export function DetailPanel({ x, y, width, height, children }: {
  x: number;
  y: number;
  width: number;
  height: number;
  children: ReactNode;
}) {
  const shadow = useId();
  return (
    <g>
      <defs>
        <filter id={shadow} x="-30%" y="-50%" width="160%" height="200%" colorInterpolationFilters="sRGB">
          <feDropShadow dx="0" dy="4" stdDeviation="6" floodColor={LIGHT.text} floodOpacity="0.07" />
        </filter>
      </defs>
      <rect x={x} y={y} width={width} height={height} rx="18" fill={LIGHT.surface}
        stroke={LIGHT.stroke} filter={`url(#${shadow})`} />
      {children}
    </g>
  );
}

export function WindowFrame({ palette, zoom = 1, active = null, children }: {
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
      <rect x="40" y="32" width="320" height="234" rx="18" fill={palette.rail} stroke={palette.stroke} />
      {[58, 68, 78].map((cx) => <circle key={cx} cx={cx} cy="49" r="2.5" fill={palette.faint} />)}
      <rect x="102" y="47" width="64" height="4" rx="2" fill={palette.faint} />
      <rect x="86" y="65" width="262" height="195" rx="14" fill={palette.surface} />
      {[91, 126, 161].map((cy, index) => (
        <g key={cy} transform={`translate(63 ${cy})`}>
          {index === active && <rect x="-14" y="-14" width="28" height="28" rx="10"
            fill={`url(#${selectedGradient})`} />}
          <g transform={`scale(${zoom})`} fill="none" stroke={index === active ? palette.sidebarIcon : palette.muted}
            strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            {index === 0 ? <path d="M-6-3H6m-3-3 3 3-3 3 M6 4H-6m3-3-3 3 3 3" />
              : <path d="M-4-3h8M-4 2h8" opacity="0.45" />}
          </g>
        </g>
      ))}
      {children}
    </>
  );
}

export function WindowContent({ palette, zoom = 1 }: { palette: typeof LIGHT; zoom?: number }) {
  return (
    <g transform={`translate(108 86) scale(${zoom})`}>
      <text y="30" fill={palette.text} fontFamily="var(--asb-font-ui)" fontSize="30" fontWeight="500">Aa</text>
      <rect x="54" y="12" width="88" height="5" rx="2.5" fill={palette.muted} />
      <rect x="54" y="25" width="56" height="4" rx="2" fill={palette.faint} />
      <rect y="58" width="174" height="30" rx="10" fill={palette.rail} />
      <circle cx="14" cy="73" r="3" fill={palette.accent} />
      <rect x="28" y="71" width="106" height="4" rx="2" fill={palette.muted} />
      <rect y="108" width="64" height="16" rx="8" fill={palette.accent} />
      <rect x="74" y="108" width="44" height="16" rx="8" fill={palette.rail} />
    </g>
  );
}

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
