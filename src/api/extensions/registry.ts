import { invoke } from "../client";
import type { McpRegistrySource } from "./mcp";
import type { ExtensionMutation } from "./types";

export interface McpRegistryEntry {
  name: string;
  version: string;
  title: string | null;
  description: string;
  docs: string | null;
  source: McpRegistrySource | null;
}

export interface McpRegistryPage {
  items: McpRegistryEntry[];
  nextCursor: string | null;
}

export interface McpRegistryConnection {
  transport: "http" | "claudeSse";
  endpoint: string;
  requiredHeaders: string[];
}

export type McpRegistryUpdateReport = { definitionId: string; revision: number } & (
  | { status: "current" }
  | { status: "available"; source: McpRegistrySource; action: "replaceEndpoint" | "associateCurrent" }
  | { status: "manual"; version: string; docs: string | null; connections: McpRegistryConnection[] }
  | { status: "failed"; message: string }
);

export type McpRegistryUpdate = Extract<McpRegistryUpdateReport, { status: "available" }>;

export function searchMcpRegistry(query: string, cursor: string | null): Promise<McpRegistryPage> {
  return invoke("search_mcp_registry", { query, cursor });
}

export function checkMcpRegistryUpdates(definitionIds: string[]): Promise<McpRegistryUpdateReport[]> {
  return invoke("check_mcp_registry_updates", { definitionIds });
}

export function applyMcpRegistryUpdate({ definitionId, revision, source, action }: McpRegistryUpdate): Promise<ExtensionMutation> {
  return invoke("apply_mcp_registry_update", { definitionId, expectedRevision: revision, source, action });
}
