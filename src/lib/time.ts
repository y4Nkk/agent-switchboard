import { currentLanguage, tr } from "../i18n/current.ts";

/**
 * Formats an RFC 3339 UTC timestamp in the machine's local timezone as
 * `YYYY年MM月DD日 HH：MM` (zh-CN) or `Sep 21, 2026 14:30` (en-US), with
 * optional seconds for sampled readings. Time views and tray timestamps
 * share this formatter.
 */
export function timeLabel(iso: string, precision: "minute" | "second" = "minute"): string {
  const date = new Date(iso);
  const [y, mo, d, h, mi] = [
    date.getFullYear(),
    date.getMonth() + 1,
    date.getDate(),
    date.getHours(),
    date.getMinutes(),
  ].map((part) => String(part).padStart(2, "0"));
  const time = precision === "second"
    ? tr("format.time.hms", { hour: h, minute: mi, second: String(date.getSeconds()).padStart(2, "0") })
    : tr("format.time.hm", { hour: h, minute: mi });
  if (currentLanguage() === "zh-CN") {
    return `${tr("format.date.full", { year: y, month: mo, day: d })} ${time}`;
  }
  const monthDay = new Intl.DateTimeFormat(currentLanguage(), {
    month: "short", day: "numeric", year: "numeric",
  }).format(date);
  return `${monthDay} ${time}`;
}

/**
 * Formats a `YYYY-MM-DD` UTC calendar date verbatim: an archive cutoff is a
 * UTC date boundary, not a local instant, so no timezone conversion happens
 * (shifting to local would fabricate a time of day and move the date in zones
 * behind UTC). Rendering goes through the `Time` component, like the other
 * formatters.
 */
export function dateLabel(iso: string): string {
  const [year, month, day] = iso.slice(0, 10).split("-");
  if (currentLanguage() === "zh-CN") {
    return tr("format.date.full", { year, month, day });
  }
  return new Intl.DateTimeFormat(currentLanguage(), {
    month: "short", day: "numeric", year: "numeric", timeZone: "UTC",
  }).format(new Date(`${iso}T00:00:00Z`));
}

/**
 * A coarse human countdown to an RFC 3339 timestamp, computed against the
 * current time at render. There is no timer: a refresh re-renders the label.
 */
export function countdownLabel(iso: string, now = Date.now()): string {
  const remaining = new Date(iso).getTime() - now;
  if (remaining <= 0) return tr("format.countdown.reached");
  const totalMinutes = Math.floor(remaining / 60_000);
  const days = Math.floor(totalMinutes / (60 * 24));
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
  const minutes = totalMinutes % 60;
  if (totalMinutes < 1) return tr("format.countdown.underMinute");
  if (days >= 1) return tr("format.countdown.daysHours", { days, hours });
  if (hours >= 1) return tr("format.countdown.hoursMinutes", { hours, minutes });
  return tr("format.countdown.minutes", { minutes });
}

/**
 * A coarse relative label for an RFC 3339 timestamp, signed by direction
 * ("8 天 3 小时前" / "8 days 3 hours ago", "45 分钟后" / "in 45 minutes"),
 * computed against the current time at render. Like the countdown label
 * there is no timer: a refresh re-renders it.
 */
export function relativeLabel(iso: string, now = Date.now()): string {
  const delta = new Date(iso).getTime() - now;
  if (Math.abs(delta) < 60_000) return tr("format.relative.justNow");
  const direction = delta >= 0 ? "format.relative.future" : "format.relative.past";
  const totalMinutes = Math.floor(Math.abs(delta) / 60_000);
  const days = Math.floor(totalMinutes / (60 * 24));
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
  const minutes = totalMinutes % 60;
  if (days >= 1) return tr(direction, { span: tr("format.relative.daysHours", { days, hours }) });
  if (hours >= 1) return tr(direction, { span: tr("format.relative.hoursMinutes", { hours, minutes }) });
  return tr(direction, { span: tr("format.relative.minutes", { minutes }) });
}
