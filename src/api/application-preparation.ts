import { invoke } from "./client";
import type { AppSettings } from "./settings";
import type { CommandError } from "./shared";

export interface ApplicationPreparation {
  sessionId: string;
  runId: number;
  revision: number;
  status: "running" | "ready" | "degraded" | "blocked";
  stage: "preferences" | "configuration" | "outboundProxy" | "services" | "complete";
  elapsedMs: number;
  settings: AppSettings | null;
  configurationReady: boolean;
  preferencesReset: boolean;
  repairedFiles: string[];
  backupPaths: string[];
  issues: CommandError[];
}

export function getApplicationPreparation(): Promise<ApplicationPreparation> {
  return invoke("get_application_preparation");
}

export function recheckApplicationData(): Promise<void> {
  return invoke("recheck_application_data");
}

export function isApplicationDataError(error: CommandError): boolean {
  return ["profile-store-unsupported", "profile-store-repair-failed", "store-unreadable",
    "outbound-proxy-unavailable", "outbound-proxy-invalid", "app-settings-unavailable"].includes(error.code);
}
