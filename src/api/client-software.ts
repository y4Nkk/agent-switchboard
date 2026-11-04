import { invoke } from "./client";
import type { AppKind } from "./shared";

export type SoftwareForm = "cli" | "desktop" | "extension" | "bundled";
export type SoftwareManager = "native" | "npm" | "homebrew" | "winget" | "system" | "manual" | "host";
export interface SoftwareRelease { version: string; source: string; checkedAt: string; stale: boolean }
export interface SoftwarePolicy {
  channel: string;
  automaticDisabled: boolean;
  updatesDisabled: boolean;
  minimum: string | null;
  maximum: string | null;
  issue: string | null;
  revision: string;
}
export type SoftwareReason = "hostManaged" | "unmanaged" | "updatesDisabled" | "policyUnavailable"
  | "releaseUnavailable" | "staleRelease" | "ahead" | "current" | "versionUnavailable"
  | "versionPolicy" | "sharedInstallation" | "administratorRequired";
export interface ClientSoftware {
  id: string;
  app: AppKind;
  form: SoftwareForm;
  manager: SoftwareManager;
  launcher: string;
  executable: string;
  root: string;
  version: string | null;
  versionText: string | null;
  active: boolean;
  channel: string;
  detail: string | null;
  official: SoftwareRelease | null;
  available: SoftwareRelease | null;
  comparison: "unknown" | "current" | "channelBehind" | "updateAvailable" | "ahead";
  policy: SoftwarePolicy;
  updateAllowed: boolean;
  reason: SoftwareReason | null;
  guideUrl: string;
  releaseError: string | null;
}
export interface ClientUpdatePlan {
  id: string;
  installationId: string;
  app: AppKind;
  launcher: string;
  manager: SoftwareManager;
  fromVersion: string;
  targetVersion: string;
  exactTarget: boolean;
  channel: string;
  proxyMode: "none" | "system" | "manual" | "wingetSystem";
  expiresAt: string;
}
export type ClientUpdatePhase = "preparing" | "runningUpdater" | "verifying" | "completed" | "failed" | "needsAttention" | "cancelled";
export interface ClientUpdateTask {
  id: string;
  revision: number;
  plan: ClientUpdatePlan;
  phase: ClientUpdatePhase;
  startedAt: string;
  finishedAt: string | null;
  actualVersion: string | null;
  cancellable: boolean;
  log: string[];
  error: string | null;
  processId: number | null;
  processBirth: string | null;
}
export interface ClientSoftwareSnapshot {
  revision: string;
  installations: ClientSoftware[];
  issues: string[];
  task: ClientUpdateTask | null;
}
export const readClientSoftware = (): Promise<ClientSoftwareSnapshot> => invoke("read_client_software");
export const selectClientSoftware = (client: AppKind): Promise<ClientSoftwareSnapshot | null> => invoke("select_client_software", { client });
export const refreshClientSoftware = (force: boolean): Promise<ClientSoftwareSnapshot> => invoke("refresh_client_software", { force });
export const prepareClientUpdate = (installationId: string, revision: string): Promise<ClientUpdatePlan> =>
  invoke("prepare_client_update", { installationId, revision });
export const startClientUpdate = (planId: string): Promise<ClientUpdateTask> => invoke("start_client_update", { planId });
export const readClientUpdate = (): Promise<ClientUpdateTask | null> => invoke("read_client_update");
export const cancelClientUpdate = (id: string): Promise<ClientUpdateTask> => invoke("cancel_client_update", { id });
export const clientUpdateActive = (task: ClientUpdateTask | null): boolean =>
  task !== null && ["preparing", "runningUpdater", "verifying"].includes(task.phase);
