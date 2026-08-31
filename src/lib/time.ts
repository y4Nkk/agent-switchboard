/**
 * Formats an RFC 3339 UTC timestamp in the machine's local timezone as
 * `YYYY-MM-DD HH:MM:SS`. Rendering goes through the `Time` component, the
 * single consumer of this formatter.
 */
export function timeLabel(iso: string): string {
  const date = new Date(iso);
  const [y, mo, d, h, mi, s] = [
    date.getFullYear(),
    date.getMonth() + 1,
    date.getDate(),
    date.getHours(),
    date.getMinutes(),
    date.getSeconds(),
  ].map((part) => String(part).padStart(2, "0"));
  return `${y}-${mo}-${d} ${h}:${mi}:${s}`;
}
