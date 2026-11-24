export interface SessionDates {
  from: string;
  to: string;
}

export type SessionTimeRange = "all" | "7" | "30" | "custom";

export const EMPTY_SESSION_DATES: SessionDates = { from: "", to: "" };

export function sessionDatesInOrder({ from, to }: SessionDates) {
  return !from || !to || from <= to;
}

export function createSessionTimeFilter(range: SessionTimeRange, dates: SessionDates) {
  if (range === "all") return { range, dates: EMPTY_SESSION_DATES, activeAfter: null, activeBefore: null };
  if (range === "custom") return {
    range, dates, activeAfter: dateBoundary(dates.from, false), activeBefore: dateBoundary(dates.to, true),
  };
  return { range, dates, activeAfter: new Date(Date.now() - Number(range) * 86400000).toISOString(), activeBefore: null };
}

export function dateBoundary(value: string, nextDay: boolean) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const date = new Date(`${value}T00:00:00`);
  if (!Number.isFinite(date.getTime())) return null;
  if (nextDay) date.setDate(date.getDate() + 1);
  if (date.getFullYear() > 9999) return null;
  return date.toISOString();
}

export function localDate(date: Date) {
  return `${date.getFullYear().toString().padStart(4, "0")}-${(date.getMonth() + 1).toString().padStart(2, "0")}-${date.getDate().toString().padStart(2, "0")}`;
}
