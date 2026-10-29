import type { ProviderProfile, WorkspacePage } from "../api/client";
import type { MessageKey } from "../i18n/messages";

/** Pages are identified by the backend `WorkspacePage` contract values, never
 * by display text; labels resolve from the catalog at render time. */
export type Page = WorkspacePage;
export const PAGES: readonly Page[] = [
  "providers", "gateway", "clientConfiguration", "extensions", "sessions", "usage", "settings",
];
export function pageLabelKey(page: Page): MessageKey {
  return `nav.page.${page}`;
}

export type ProviderView =
  | { kind: "list" }
  | { kind: "import" }
  | { kind: "usage"; profile: ProviderProfile };

export const SETTINGS_SECTIONS = [
  { value: "application", labelKey: "nav.section.application" },
  { value: "client-management", labelKey: "nav.section.clientManagement" },
  { value: "backups", labelKey: "nav.section.backups" },
  { value: "storage", labelKey: "nav.section.storage" },
  { value: "diagnostics", labelKey: "nav.section.diagnostics" },
  { value: "about", labelKey: "nav.section.about" },
] as const satisfies ReadonlyArray<{ value: SettingsSection; labelKey: MessageKey }>;
export type SettingsSection =
  | "application" | "client-management" | "backups" | "storage" | "diagnostics" | "about";

export const EXTENSION_SECTIONS = [
  { value: "skill", labelKey: "nav.section.skill" },
  { value: "mcp", labelKey: "nav.section.mcp" },
] as const satisfies ReadonlyArray<{ value: ExtensionSection; labelKey: MessageKey }>;
export type ExtensionSection = "skill" | "mcp";

export const USAGE_SECTIONS = [
  { value: "consumption", labelKey: "nav.section.consumption" },
  { value: "quota", labelKey: "nav.section.quota" },
  { value: "resources", labelKey: "nav.section.resources" },
  { value: "radar", labelKey: "nav.section.radar" },
] as const satisfies ReadonlyArray<{ value: UsageSection; labelKey: MessageKey }>;
export type UsageSection = "consumption" | "quota" | "resources" | "radar";

export const DIAGNOSTIC_SECTIONS = [
  { value: "configuration", labelKey: "nav.section.configuration" },
  { value: "logs", labelKey: "nav.section.logs" },
] as const satisfies ReadonlyArray<{ value: DiagnosticSection; labelKey: MessageKey }>;
export type DiagnosticSection = "configuration" | "logs";
