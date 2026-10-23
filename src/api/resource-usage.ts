import { invoke } from "./client";

export type ResourceGroup = "switchboard" | "codex" | "claude";
export type ResourceIssue =
  | "warmingUp" | "accessDenied" | "processExited" | "identityUnavailable" | "identityChanged"
  | "counterReset" | "intervalInterrupted" | "cpuTopologyChanged" | "collectionFailed"
  | "unrecognizedExecutable" | "webviewUnavailable" | "webviewPartial" | "browserExcluded"
  | "enumerationRestricted" | "namespaceBoundary" | "approximateRss" | "parentUnverified" | "notObserved";

export interface ResourceMetric {
  state: "measured" | "partial" | "unavailable";
  value: number | null;
  measuredProcesses: number;
  missingProcesses: number;
  issues: ResourceIssue[];
}

export interface ResourceGroupReading {
  group: ResourceGroup;
  presence: "running" | "notObserved" | "indeterminate";
  cpu: ResourceMetric;
  residentBytes: ResourceMetric;
  processCount: ResourceMetric;
  issues: ResourceIssue[];
}

export interface ResourceProcess {
  key: string;
  pid: number;
  name: string;
  role: "application" | "engine" | "interface" | "task";
  evidence: "application" | "executable" | "ancestor" | "webview" | "retained";
  cpu: ResourceMetric;
  residentBytes: ResourceMetric;
}

export interface ResourceSelection { group: ResourceGroup; page: number }
export interface ResourceReadRequest { resetInterval: boolean; details: ResourceSelection | null }
export interface ResourceDetails {
  group: ResourceGroup;
  page: number;
  pageSize: number;
  total: number;
  rows: ResourceProcess[];
}

export interface ResourceRead {
  sampleId: number;
  sampledAt: string;
  nextReadInMs: number;
  scope: {
    platform: string;
    interfaceCoverage: "verified" | "partial" | "browserExcluded";
    issues: ResourceIssue[];
  };
  groups: ResourceGroupReading[];
  details: ResourceDetails | null;
}

export function readResourceUsage(request: ResourceReadRequest): Promise<ResourceRead> {
  return invoke<ResourceRead>("read_resource_usage", { request });
}
