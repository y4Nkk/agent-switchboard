import type { UsageQuery } from "../api/client";

function optional(value: string): string | null {
  const normalized = value.trim();
  return normalized || null;
}

/** An untouched optional query is persisted as null. */
export function normalizeUsageQuery(query: UsageQuery | null | undefined): UsageQuery | null {
  if (!query) return null;
  if (query.kind === "script") {
    const source = query.source.trim();
    return source ? { kind: "script", source } : null;
  }

  const url = optional(query.url);
  const hasPath = [query.remainingPath, query.usedPath, query.totalPath].some(
    (path) => path !== null && path !== undefined && path.trim(),
  );
  if (!url && !hasPath) return null;

  return {
    kind: "declarative",
    url: url ?? "",
    remainingPath: optional(query.remainingPath ?? ""),
    usedPath: optional(query.usedPath ?? ""),
    totalPath: optional(query.totalPath ?? ""),
    unit: optional(query.unit ?? ""),
  };
}
