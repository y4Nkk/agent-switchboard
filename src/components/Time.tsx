import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { countdownLabel, dateLabel, relativeLabel, timeLabel } from "../lib/time";

/** Absolute precision follows the supplied timestamp; it never creates a clock.
 * "date" shows a UTC calendar date verbatim. Relative views tick while visible. */
export function Time({ iso, mode = "absolute", precision = "minute" }: {
  iso: string; mode?: "absolute" | "date" | "relative" | "reset"; precision?: "minute" | "second";
}) {
  useI18n();
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (mode === "absolute" || mode === "date") return;
    let timer: ReturnType<typeof setInterval> | undefined;
    const sync = () => {
      clearInterval(timer);
      if (document.hidden) return;
      setNow(Date.now());
      timer = setInterval(() => setNow(Date.now()), 30_000);
    };
    sync();
    document.addEventListener("visibilitychange", sync);
    return () => { clearInterval(timer); document.removeEventListener("visibilitychange", sync); };
  }, [mode]);
  const label = mode === "relative" ? relativeLabel(iso, now)
    : mode === "reset" ? countdownLabel(iso, now)
    : mode === "date" ? dateLabel(iso) : timeLabel(iso, precision);
  return <time dateTime={iso} title={mode === "relative" || mode === "reset" ? timeLabel(iso) : undefined}>{label}</time>;
}
