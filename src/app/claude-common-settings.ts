export type ClaudeExtraSettings = Record<string, unknown> | undefined;
function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => [key, canonical(item)]));
  return value;
}
export function sameClaudeExtra(left: ClaudeExtraSettings, right: ClaudeExtraSettings): boolean {
  return JSON.stringify(canonical(left ?? {})) === JSON.stringify(canonical(right ?? {}));
}
