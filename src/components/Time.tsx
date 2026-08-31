import { timeLabel } from "../lib/time";

/** The single timestamp renderer: every time display in the interface goes
 *  through this component so the format stays uniform (local timezone,
 *  no offset suffix — the clock already reads as local time). */
export function Time({ iso }: { iso: string }) {
  return <time dateTime={iso}>{timeLabel(iso)}</time>;
}
