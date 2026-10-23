import { useId, type CSSProperties, type ReactNode } from "react";
import type { CloseBehavior, MotionPreference, RuntimeLogLevel, RuntimeLogSeverity, StartupPage } from "../api/client";
import { DARK, LIGHT, PreviewCanvas, WindowContent, WindowFrame } from "./preference-preview/WindowScene";

export function AppPreviewSetting<T extends string | number>({ label, detail, value, options, renderPreview, busy, onChange }: {
  label: string;
  detail?: string;
  value: T;
  options: readonly { value: T; label: string; description: string }[];
  renderPreview: (value: T) => ReactNode;
  busy: boolean;
  onChange: (value: T) => void;
}) {
  const id = useId();
  return (
    <div className="asb-app-setting-row asb-app-setting-row-preview">
      <div className="asb-app-setting-copy">
        <span id={`${id}-label`} className="asb-checkbox-label">{label}</span>
        {detail && <span id={`${id}-detail`} className="asb-app-setting-detail">{detail}</span>}
      </div>
      <div className="asb-preview-cards" role="radiogroup" aria-labelledby={`${id}-label`}
        aria-describedby={detail ? `${id}-detail` : undefined}>
        {options.map((option, index) => (
          <label key={option.value} className="asb-preview-card">
            <input type="radio" name={id} checked={value === option.value} disabled={busy}
              aria-labelledby={`${id}-${index}-name`} aria-describedby={`${id}-${index}-description`}
              onChange={() => onChange(option.value)} />
            <span className="asb-preview-card-art" aria-hidden="true">{renderPreview(option.value)}</span>
            <span className="asb-preview-card-heading">
              <span id={`${id}-${index}-name`} className="asb-preview-card-name">{option.label}</span>
              <svg className="asb-preview-card-check" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
                <circle cx="8" cy="8" r="7" fill="currentColor" />
                <path d="m4.5 8 2.2 2.2 4.8-4.6" fill="none" stroke="var(--asb-text-on-action)"
                  strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </span>
            <span id={`${id}-${index}-description`} className="asb-preview-card-description">{option.description}</span>
          </label>
        ))}
      </div>
    </div>
  );
}

export function CloseBehaviorPreview({ behavior }: { behavior: CloseBehavior }) {
  const exit = behavior === "exit";
  return (
    <PreviewCanvas>
      <g transform="translate(2 0) scale(0.78)" opacity="0.38">
        <WindowFrame palette={LIGHT}><WindowContent palette={LIGHT} /></WindowFrame>
      </g>
      <rect x="8" y="83" width="144" height="17" rx="5" fill={LIGHT.rail} stroke={LIGHT.stroke} />
      <path d="M17 90h5m-5 4h5 M123 91h3m4 0h3" stroke={LIGHT.muted} strokeWidth="1.5" strokeLinecap="round" />
      {exit ? (
        <g fill="none" stroke={LIGHT.text} strokeWidth="3" strokeLinecap="round">
          <circle cx="103" cy="56" r="21" fill={LIGHT.surface} stroke={LIGHT.stroke} strokeWidth="1" />
          <path d="M103 42v13 M95 47a12 12 0 1 0 16 0" />
        </g>
      ) : (
        <>
          <path d="M100 59c24 0 24 17 37 17v8m-4-4 4 4 4-4" fill="none"
            stroke={LIGHT.accent} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          <rect x="136" y="87" width="10" height="10" rx="3" fill={LIGHT.accent} />
          <path d="m139 92 2 2 3-4" fill="none" stroke={LIGHT.surface} strokeWidth="1.2" />
        </>
      )}
    </PreviewCanvas>
  );
}

export function MotionPreview({ mode }: { mode: MotionPreference }) {
  const resting = mode === "reduce";
  const track = { start: 49, end: 109, y: 66 };
  const travelStyle: CSSProperties & { "--asb-preview-travel": string } = {
    "--asb-preview-travel": `${track.end - track.start}px`,
  };
  return (
    <PreviewCanvas>
      <WindowFrame palette={LIGHT} active={null}>
        <rect x="42" y="32" width="38" height="4" rx="2" fill={LIGHT.muted} />
        <rect x="42" y="41" width="62" height="3" rx="1.5" fill={LIGHT.faint} />
        <rect x={track.start - 3} y={track.y - 3} width={track.end - track.start + 6} height="6" rx="3" fill={LIGHT.faint} />
        <circle cx={track.start} cy={track.y} r="6" fill={LIGHT.accent} style={travelStyle}
          className={resting ? undefined : "asb-motion-demo-dot"} />
        {resting ? (
          <path d="M128 60v12m6-12v12" stroke={LIGHT.muted} strokeWidth="3" strokeLinecap="round" />
        ) : (
          <g fill="none" stroke={LIGHT.muted} strokeWidth="1.5" strokeLinecap="round">
            <rect x="122" y="59" width="16" height="11" rx="2" />
            <path d="M130 70v4m-5 0h10" />
          </g>
        )}
      </WindowFrame>
    </PreviewCanvas>
  );
}

const LOG_LADDER = [
  { level: "debug", bar: 72 }, { level: "info", bar: 58 },
  { level: "warn", bar: 64 }, { level: "error", bar: 48 },
] as const satisfies ReadonlyArray<{ level: RuntimeLogSeverity; bar: number }>;

const LOG_RANK: Record<RuntimeLogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3, silent: 4 };
const LOG_COLORS: Record<RuntimeLogSeverity, string> = {
  debug: DARK.accent, info: DARK.muted, warn: DARK.warning, error: DARK.danger,
};

export function LogLevelPreview({ level }: { level: RuntimeLogLevel }) {
  const rank = LOG_RANK[level];
  return (
    <PreviewCanvas>
      <rect x="4" y="4" width="152" height="96" rx="10" fill={DARK.surface} stroke={DARK.stroke} />
      <path d="m14 13 4 3-4 3m9 0h6" fill="none" stroke={DARK.muted} strokeWidth="1.5" strokeLinecap="round" />
      <rect x="121" y="13" width="23" height="4" rx="2" fill={DARK.faint} />
      {level !== "silent" ? LOG_LADDER.map((line, index) => {
        if (LOG_RANK[line.level] < rank) return null;
        const y = 29 + index * 17;
        const color = LOG_COLORS[line.level];
        return (
          <g key={line.level}>
            <circle cx="17" cy={y + 4} r="2" fill={color} />
            <rect x="25" y={y} width="20" height="8" rx="3" fill={color} />
            <rect x="53" y={y + 2} width={line.bar} height="4" rx="2" fill={DARK.muted} />
          </g>
        );
      }) : (
        <g fill="none" stroke={DARK.muted} strokeWidth="1.5" strokeLinecap="round">
          <circle cx="80" cy="53" r="12" />
          <path d="m72 61 16-16 M60 77h40" />
        </g>
      )}
    </PreviewCanvas>
  );
}

export function StartupPagePreview({ page }: { page: StartupPage }) {
  const providers = page === "providers";
  return (
    <PreviewCanvas>
      <WindowFrame palette={LIGHT} active={providers ? 0 : null}>
        {providers ? (
          <>
            <rect x="40" y="32" width="38" height="4" rx="2" fill={LIGHT.text} />
            {[46, 67].map((y, index) => (
              <g key={y}>
                <rect x="40" y={y} width="102" height="17" rx="5" fill={LIGHT.rail} />
                <circle cx="49" cy={y + 8.5} r="3" fill={index === 0 ? LIGHT.accent : LIGHT.muted} />
                <rect x="58" y={y + 5} width={index === 0 ? 40 : 30} height="3" rx="1.5" fill={LIGHT.muted} />
                <rect x="58" y={y + 11} width="22" height="2" rx="1" fill={LIGHT.faint} />
              </g>
            ))}
          </>
        ) : (
          <>
            <path d="M64 46v-9l-9 9 9 9v-9h14a15 15 0 1 1-11 25" fill="none"
              stroke={LIGHT.accent} strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
            <circle cx="113" cy="62" r="13" fill={LIGHT.surface} stroke={LIGHT.muted} strokeWidth="1.5" />
            <path d="M113 54v8l5 3" fill="none" stroke={LIGHT.text} strokeWidth="1.5" strokeLinecap="round" />
            <rect x="62" y="86" width="58" height="3" rx="1.5" fill={LIGHT.faint} />
          </>
        )}
      </WindowFrame>
    </PreviewCanvas>
  );
}
