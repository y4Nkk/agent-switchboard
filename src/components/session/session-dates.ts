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
