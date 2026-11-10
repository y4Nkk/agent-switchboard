import { useId, type CSSProperties, type ReactNode } from "react";
import type { CloseBehavior, MotionPreference, RuntimeLogLevel, RuntimeLogSeverity, StartupPage } from "../api/client";
import { AppMark, DetailPanel, LIGHT, PreviewCanvas, WindowFrame } from "./preference-preview/WindowScene";

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
        style={{ "--asb-preview-columns": options.length === 4 ? 2 : Math.min(options.length, 3) } as CSSProperties}
        aria-describedby={detail ? `${id}-detail` : undefined}>
        {options.map((option, index) => (
          <label key={option.value} className="asb-preview-card">
            <input type="radio" name={id} checked={value === option.value} disabled={busy}
              aria-labelledby={`${id}-${index}-name`} aria-describedby={`${id}-${index}-description`}
              onChange={() => onChange(option.value)} />
            <span className="asb-preview-card-art" aria-hidden="true">{renderPreview(option.value)}</span>
            <span className="asb-preview-card-caption">
              <span id={`${id}-${index}-name`} className="asb-preview-card-name">{option.label}</span>
              <span id={`${id}-${index}-description`} className="asb-preview-card-description">{option.description}</span>
            </span>
            <svg className="asb-preview-card-check" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <circle cx="12" cy="12" r="11" fill="currentColor" stroke="var(--asb-content)" strokeWidth="2" />
              <path d="m7 12 3.2 3.2 6.8-6.6" fill="none" stroke="var(--asb-text-on-action)"
                strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
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
      <g opacity={exit ? 0.3 : 0.55}>
        <WindowFrame palette={LIGHT}>
          <rect x="108" y="94" width="108" height="5" rx="2.5" fill={LIGHT.faint} />
          <rect x="108" y="108" width="72" height="4" rx="2" fill={LIGHT.faint} />
        </WindowFrame>
      </g>
      <path d="m331 45 7 7m0-7-7 7" stroke={LIGHT.muted} strokeWidth="1.3" strokeLinecap="round" />
      {exit ? (
        <DetailPanel x={140} y={100} width={120} height={88}>
          <path d="M200 116v19 M186 122a22 22 0 1 0 28 0" fill="none"
            stroke={LIGHT.muted} strokeWidth="2.5" strokeLinecap="round" />
        </DetailPanel>
      ) : (
        <>
          <path d="M334 64v25c0 25-72 25-72 54v9m-4-4 4 4 4-4" fill="none"
            stroke={LIGHT.faint} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          <DetailPanel x={134} y={160} width={132} height={48}>
            <AppMark x={152} y={172} size={24} />
            <rect x="190" y="175" width="48" height="4" rx="2" fill={LIGHT.muted} />
            <rect x="190" y="185" width="30" height="3" rx="1.5" fill={LIGHT.faint} />
          </DetailPanel>
        </>
      )}
    </PreviewCanvas>
  );
}

export function MotionPreview({ mode }: { mode: MotionPreference }) {
  const resting = mode === "reduce";
  const track = { start: 110, end: 254, y: 144 };
  const travelStyle: CSSProperties & { "--asb-preview-travel": string } = {
    "--asb-preview-travel": `${track.end - track.start}px`,
  };
  return (
    <PreviewCanvas>
      <WindowFrame palette={LIGHT}>
        <rect x="108" y="94" width="108" height="5" rx="2.5" fill={LIGHT.faint} />
      </WindowFrame>
      <DetailPanel x={80} y={112} width={240} height={64}>
        <rect x={track.start} y={track.y - 2} width={track.end - track.start} height="4" rx="2" fill={LIGHT.rail} />
        <circle cx={track.end} cy={track.y} r="7" fill={LIGHT.surface} stroke={LIGHT.stroke} />
        <circle cx={track.start} cy={track.y} r="7" fill={LIGHT.accent} style={travelStyle}
          className={resting ? undefined : "asb-motion-demo-dot"} />
        {resting ? (
          <path d="M287 138v12m6-12v12" stroke={LIGHT.muted} strokeWidth="2" strokeLinecap="round" />
        ) : (
          <g fill="none" stroke={LIGHT.muted} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="m287 138 6 6-6 6" />
          </g>
        )}
      </DetailPanel>
    </PreviewCanvas>
  );
}

const LOG_LADDER = [
  { level: "debug", bar: 144 }, { level: "info", bar: 118 },
  { level: "warn", bar: 132 }, { level: "error", bar: 98 },
] as const satisfies ReadonlyArray<{ level: RuntimeLogSeverity; bar: number }>;

const LOG_RANK: Record<RuntimeLogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3, silent: 4 };
const LOG_COLORS: Record<RuntimeLogSeverity, string> = {
  debug: LIGHT.accent, info: LIGHT.muted, warn: LIGHT.warning, error: LIGHT.danger,
};

export function LogLevelPreview({ level }: { level: RuntimeLogLevel }) {
  const rank = LOG_RANK[level];
  return (
    <PreviewCanvas>
      <WindowFrame palette={LIGHT}>
        {level !== "silent" ? LOG_LADDER.map((line, index) => {
          if (LOG_RANK[line.level] < rank) return null;
          const y = 106 + index * 28;
          const color = LOG_COLORS[line.level];
          return (
            <g key={line.level}>
              <circle cx="108" cy={y} r="2.5" fill={color} />
              <text x="120" y={y + 4} fill={color} fontFamily="var(--asb-font-code)" fontSize="11">{line.level}</text>
              <rect x="176" y={y - 2} width={line.bar} height="4" rx="2" fill={LIGHT.faint} />
            </g>
          );
        }) : (
          <g fill="none" stroke={LIGHT.muted} strokeWidth="1.5" strokeLinecap="round">
            <circle cx="216" cy="144" r="20" />
            <path d="m202 158 28-28 M191 188h50" />
          </g>
        )}
      </WindowFrame>
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
            <rect x="108" y="89" width="88" height="5" rx="2.5" fill={LIGHT.muted} />
            {[114, 174].map((y, index) => (
              <g key={y}>
                <rect x="104" y={y} width="226" height="42" rx="10" fill={LIGHT.rail} />
                <circle cx="128" cy={y + 21} r="5" fill={LIGHT.faint} />
                <text x="148" y={y + 18} fill={LIGHT.muted} fontFamily="var(--asb-font-ui)" fontSize="12" fontWeight="500">
                  {index === 0 ? "Codex" : "Claude Code"}
                </text>
                <rect x="148" y={y + 28} width={index === 0 ? 102 : 74} height="3" rx="1.5" fill={LIGHT.faint} />
              </g>
            ))}
          </>
        ) : (
          <>
            <path d="M190 124v-10l-10 10 10 10v-10h8" fill="none" stroke={LIGHT.faint}
              strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            <circle cx="224" cy="148" r="25" fill="none" stroke={LIGHT.muted} strokeWidth="1.5" />
            <path d="M224 134v14l10 6" fill="none" stroke={LIGHT.accent} strokeWidth="2" strokeLinecap="round" />
          </>
        )}
      </WindowFrame>
    </PreviewCanvas>
  );
}
