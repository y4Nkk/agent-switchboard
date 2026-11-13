import type { Dispatch, SetStateAction } from "react";
import type { AppKind, CodexProviderRecord, ProviderRecord, ProviderConversion } from "../api/client";
import type { RouteInventory, RouteRole } from "../components/model-routing/model";

export type ProviderEditorSection = "connection" | "models" | "parameters";
export interface ProviderEditorContext {
  instanceId: string;
  profileId: string;
  section: ProviderEditorSection;
  role?: RouteRole;
  returnTo: "providers" | "gateway";
}

export interface ProviderEditorRequest extends Omit<ProviderEditorContext, "instanceId"> {
  app: AppKind;
  revision: number;
  reload: boolean;
}

export interface ProviderEditorNavigation {
  request: ProviderEditorRequest | null;
  onRequestHandled: (revision: number) => void;
  onReplaceSession: (request: ProviderEditorRequest) => void;
  onReload: (section: ProviderEditorSection, role: RouteRole) => void;
  inventory: RouteInventory;
}

export function createEditorContext(profileId: string = crypto.randomUUID()): ProviderEditorContext {
  return { profileId, instanceId: crypto.randomUUID(), section: "connection", returnTo: "providers" };
}

/** Existing identities and detached conversion seeds take distinct save paths. */
export type CodexEditorSource =
  | { kind: "record"; record: CodexProviderRecord }
  | { kind: "official"; record: ProviderRecord | null }
  | { kind: "conversion"; conversion: ProviderConversion; sessionId: string };

/** One open editor. Claude owns the generic contract; Codex owns its strict
 * specialized contract — the two draft shapes never convert into each other. */
export type ProviderEditorSession = ProviderEditorContext & (
  | { app: "claude"; record: ProviderRecord | null; conversion?: never; sessionId?: never }
  | { app: "claude"; record: null; conversion: ProviderConversion; sessionId: string }
  | { app: "codex"; source: CodexEditorSource | null });

export type SetEditorSession = Dispatch<SetStateAction<ProviderEditorSession | null>>;
