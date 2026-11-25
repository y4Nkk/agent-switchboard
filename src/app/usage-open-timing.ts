import { useLayoutEffect, useRef } from "react";
import type { UsageSection } from "./navigation";

let pending: { section: UsageSection; startedAt: number } | null = null;

export function markUsageOpen(section: UsageSection) {
  pending = { section, startedAt: performance.now() };
}

function afterPaint(measure: () => void) {
  let frame = requestAnimationFrame(() => { frame = requestAnimationFrame(measure); });
  return () => cancelAnimationFrame(frame);
}

function measureOpen(section: UsageSection, phase: "feedback" | "data", startedAt: number) {
  const name = `usage:${section}:${phase}`;
  if (performance.getEntriesByName(name, "measure").length >= 100) performance.clearMeasures(name);
  performance.measure(name, { start: startedAt });
}

export function useUsageOpenTiming(section: UsageSection | null, dataReady: boolean) {
  const visit = useRef<{ section: UsageSection; startedAt: number; measuredData: boolean } | null>(null);
  useLayoutEffect(() => {
    if (section === null) { visit.current = null; return; }
    const previous = visit.current;
    const startedAt = pending?.section === section ? pending.startedAt
      : previous?.section === section ? previous.startedAt : performance.now();
    pending = null;
    visit.current = { section, startedAt, measuredData: false };
    return afterPaint(() => measureOpen(section, "feedback", startedAt));
  }, [section]);
  useLayoutEffect(() => {
    const current = visit.current;
    if (section === null || !dataReady || !current || current.measuredData) return;
    return afterPaint(() => {
      current.measuredData = true;
      measureOpen(section, "data", current.startedAt);
    });
  }, [section, dataReady]);
}
