import type { SecretValue } from "./types";

export interface McpRegistrySource {
  name: string;
  version: string;
  endpoint: string;
}

export interface McpMetadata {
  displayName?: string | null;
  description?: string | null;
  tags?: string[];
  homepage?: string | null;
  docs?: string | null;
  registry?: McpRegistrySource | null;
}

export interface CodexServerOptions {
  cwd?: string | null;
  startupTimeoutSec?: number | null;
  toolTimeoutSec?: number | null;
  required?: boolean | null;
}

export type McpDefinition =
  | {
      transport: "stdio";
      command: string;
      args?: string[];
      env?: Record<string, SecretValue>;
      codexOptions?: CodexServerOptions | null;
    }
  | {
      transport: "http";
      url: string;
      headers?: Record<string, SecretValue>;
      bearer?: SecretValue | null;
    }
  | {
      transport: "claudeSse";
      url: string;
      headers?: Record<string, SecretValue>;
    }
  | {
      transport: "claudeWs";
      url: string;
      headers?: Record<string, SecretValue>;
    };

/** One editable secret-bearing position as prefilled for the editor. Stored
 * credentials are reduced to a presence marker; their values and handles
 * never cross the IPC boundary back to the renderer. */
export type SecretSlotView =
  | { mode: "plain"; value: string }
  | { mode: "envRef"; name: string }
  | { mode: "secretConfigured" };

export interface SecretSlot {
  name: string;
  value: SecretSlotView;
}

export type McpEditView =
  | {
      transport: "stdio";
      command: string;
      args: string[];
      env: SecretSlot[];
      codexOptions: CodexServerOptions | null;
    }
  | {
      transport: "http";
      url: string;
      headers: SecretSlot[];
      bearer: SecretSlotView | null;
    }
  | { transport: "claudeSse"; url: string; headers: SecretSlot[] }
  | { transport: "claudeWs"; url: string; headers: SecretSlot[] };

export type McpEditViewEnvelope = {
  id: string;
  revision: number;
  name: string;
  mcpMetadata?: McpMetadata | null;
} & McpEditView;

/** One field position's explicit treatment in an edit: absent keeps the
 * stored value verbatim (kept values never round-trip through the
 * renderer), replace overwrites, delete removes optional positions. */
export type FieldEdit<T> = { action: "replace"; value: T } | { action: "delete" };

export interface McpFieldEdits {
  command?: FieldEdit<string>;
  args?: FieldEdit<string[]>;
  env?: Record<string, FieldEdit<SecretValue>>;
  url?: FieldEdit<string>;
  headers?: Record<string, FieldEdit<SecretValue>>;
  bearer?: FieldEdit<SecretValue>;
  codexOptions?: FieldEdit<CodexServerOptions>;
}

export interface McpEditRequest {
  expectedRevision: number;
  serverKey?: string;
  mcpMetadata?: FieldEdit<McpMetadata>;
  transport?: McpDefinition;
  fields?: McpFieldEdits;
}
