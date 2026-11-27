import { memo, useId, useMemo, useState, type CSSProperties, type KeyboardEvent } from "react";
import type { ModelUsageReport } from "../api/model-usage";
import { useI18n, type MessageKey } from "../i18n";
import { TOKEN_UNIT } from "../lib/token-format";
import { activityWeeks, calendarDate, hasKnownUsageTotal } from "./model-usage-display";
import { RadioOption } from "./RadioOption";

const MODES = {
  daily: "usage.activity.daily",
  weekly: "usage.activity.weekly",
  cumulative: "usage.activity.cumulative",
} as const satisfies Record<string, MessageKey>;

type Mode = keyof typeof MODES;
const MODE_OPTIONS = Object.keys(MODES) as Mode[];
type Weeks = ReturnType<typeof activityWeeks>;
type ActivityCell = { day: number; end?: number; tokens: number | null };

function cellsForMode(weeks: Weeks, mode: Mode): ActivityCell[][] {
  if (mode === "daily") return weeks;
  let cumulative = 0;
  return weeks.map((week) => {
    const observed = week.filter(({ tokens }) => tokens !== null);
    const total = observed.reduce((sum, { tokens }) => sum + (tokens ?? 0), 0);
    cumulative += total;
    return [{ day: week[0].day, end: observed[observed.length - 1].day,
      tokens: mode === "weekly" ? total : cumulative }];
  });
}

function activityLabels(weeks: Weeks, language: string) {
  const month = new Intl.DateTimeFormat(language, { month: "short", timeZone: "UTC" });
  const labels = weeks.flatMap((week, index) => {
    const newMonth = week.find(({ day, tokens }) => tokens !== null && calendarDate(day).getUTCDate() === 1);
    const day = newMonth ?? (index === 0 ? week[0] : undefined);
    return day ? [{ column: index + 1, label: month.format(calendarDate(day.day)) }] : [];
  });
  return labels[1]?.column === 2 ? labels.slice(1) : labels;
}

function activityFill(tokens: number, max: number, mode: Mode): CSSProperties | undefined {
  if (tokens <= 0) return undefined;
  const ratio = tokens / max;
  if (mode === "daily") {
    const strength = [23, 41, 66, 92][Math.ceil(Math.sqrt(ratio) * 4) - 1];
    return { "--asb-activity-fill": `${strength}%` } as CSSProperties;
  }
  return { "--asb-activity-height": `${ratio * 100}%` } as CSSProperties;
}

function activityPlot(weeks: Weeks, mode: Mode, language: string, known: boolean) {
  const columns = cellsForMode(weeks, mode);
  const cells = columns.flat();
  const max = Math.max(...cells.map(({ tokens }) => tokens ?? 0));
  const date = new Intl.DateTimeFormat(language, { dateStyle: "medium", timeZone: "UTC" });
  const counts = new Intl.NumberFormat(language);
  const plot = columns.map((column) => column.map(({ day, end, tokens }) => {
    if (tokens === null) return { day, reading: null };
    const period = end === undefined ? date.format(calendarDate(day)) : date.formatRange(calendarDate(day), calendarDate(end));
    const value = known ? `${counts.format(tokens)} ${TOKEN_UNIT}` : "—";
    return { day, reading: `${period} · ${value}`, style: activityFill(tokens, max, mode),
      bar: mode !== "daily" && tokens > 0 };
  }));
  return { columns: plot, readings: plot.flat(), lastIndex: cells.filter(({ tokens }) => tokens !== null).length - 1 };
}

function handleGridKey(event: KeyboardEvent<HTMLDivElement>, stride: number, lastIndex: number) {
  if (!(event.target instanceof HTMLElement) || event.target.dataset.cell === undefined) return;
  const index = Number(event.target.dataset.cell);
  const row = index % stride;
  let next: number;
  switch (event.key) {
    case "Home": next = 0; break;
    case "End": next = lastIndex; break;
    case "ArrowLeft": next = index - stride; break;
    case "ArrowRight": next = index + stride; break;
    case "ArrowUp": next = row > 0 ? index - 1 : index; break;
    case "ArrowDown": next = row < stride - 1 ? index + 1 : index; break;
    default: return;
  }
  event.preventDefault();
  const target = Math.min(next, lastIndex);
  if (target !== index) event.currentTarget.querySelector<HTMLElement>(`[data-cell="${target}"]`)?.focus();
}

function ActivityPlot({ weeks, mode, label, known }: { weeks: Weeks; mode: Mode; label: string; known: boolean }) {
  const { language } = useI18n();
  const { columns, readings, lastIndex } = useMemo(() => activityPlot(weeks, mode, language, known), [weeks, mode, language, known]);
  const monthLabels = useMemo(() => activityLabels(weeks, language), [weeks, language]);
  const [cursor, setCursor] = useState<number | null>(null);
  const [focused, setFocused] = useState(false);
  const [hovered, setHovered] = useState<number | null>(null);
  const stride = columns[0].length;
  const tabStop = Math.min(cursor ?? lastIndex, lastIndex);
  const reading = readings[hovered ?? (focused ? tabStop : -1)]?.reading;
  return <div className="asb-token-activity-plot" style={{ "--asb-activity-columns": columns.length } as CSSProperties}>
    <div className="asb-token-activity-grid" data-mode={mode} role="group" aria-label={label}
      onKeyDown={(event) => handleGridKey(event, stride, lastIndex)}
      onPointerOver={(event) => {
        const cell = event.target instanceof HTMLElement ? event.target.dataset.cell : undefined;
        setHovered(cell === undefined ? null : Number(cell));
      }}
      onPointerLeave={() => setHovered(null)}
      onFocus={(event) => {
        setCursor(Number(event.target.dataset.cell));
        setFocused(true);
      }}
      onBlur={() => setFocused(false)}>
      {columns.map((column, index) => <div key={column[0].day} className="asb-token-activity-week">{column.map((cell, row) => {
        if (cell.reading === null) return <span key={cell.day} className="asb-token-activity-cell" data-future="true" />;
        const cellIndex = index * stride + row;
        return <span key={cell.day} className="asb-token-activity-cell" role="img" aria-label={cell.reading}
          data-cell={cellIndex} data-bar={cell.bar || undefined} tabIndex={cellIndex === tabStop ? 0 : -1} style={cell.style} />;
      })}</div>)}
    </div>
    <div className="asb-token-activity-months" aria-hidden="true">
      {monthLabels.map(({ column, label }) => <span key={column} style={{ gridColumn: column }}
        data-end={column > columns.length - 2 || undefined}>{label}</span>)}
    </div>
    {reading && <span className="asb-usage-chart-tooltip asb-token-activity-reading" aria-hidden="true">{reading}</span>}
  </div>;
}

export const ModelUsageActivity = memo(function ModelUsageActivity({ report }: { report: ModelUsageReport }) {
  const { t } = useI18n();
  const group = useId();
  const [mode, setMode] = useState<Mode>("daily");
  const weeks = useMemo(() => activityWeeks(report), [report]);
  const known = hasKnownUsageTotal(report) && (report.days.length > 0 || report.groups.length === 0);
  return <section className="asb-consumption-card asb-token-activity" aria-labelledby={`${group}-title`}>
    <div className="asb-consumption-row">
      <h4 id={`${group}-title`}>{t("usage.activity.title")}</h4>
      <div className="asb-segments asb-consumption-segments" role="radiogroup" aria-label={t("usage.activity.view")}>
        {MODE_OPTIONS.map((option) => <RadioOption key={option} name={group} checked={mode === option}
          label={t(MODES[option])} disabled={false} onChange={() => setMode(option)} />)}
      </div>
    </div>
    <ActivityPlot key={mode} weeks={weeks} mode={mode} label={t(MODES[mode])} known={known} />
  </section>;
});
