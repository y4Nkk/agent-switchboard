import type { AppKind } from "../api/client";

/** Single owner of the AppKind display name used across pages and sheets. */
export function clientName(app: AppKind): string {
  return app === "codex" ? "Codex" : "Claude";
}
