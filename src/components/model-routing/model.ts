import type { AppKind, CodexProviderDraft, CodexProviderRecord, ProviderDraft, ProviderRecord } from "../../api/client";
import { claudeOptions, codexProviderDraft, providerDraft } from "../../lib/provider-drafts.ts";
import type { MessageKey } from "../../i18n";

export type RouteRole = "main" | "subagent" | "haiku" | "sonnet" | "opus" | "fable";
export type RouteDocument =
  | { kind: "codex"; record: CodexProviderRecord; draft: CodexProviderDraft }
  | { kind: "profile"; record: ProviderRecord; draft: ProviderDraft };
export type RouteRecord =
  | { kind: "codex"; record: CodexProviderRecord }
  | { kind: "profile"; record: ProviderRecord };

export interface RouteInventory {
  codex: CodexProviderRecord[];
  profiles: ProviderRecord[];
  activeIds: Partial<Record<AppKind, string | null>>;
}

export interface ModelRouteRequest { app: AppKind; profileId: string; revision: number }

export function routeDocument(source: RouteRecord): RouteDocument {
  return source.kind === "codex"
    ? { ...source, draft: codexProviderDraft(source.record) }
    : { ...source, draft: providerDraft(source.record) };
}

export function routeRecords(inventory: RouteInventory, app: AppKind): RouteRecord[] {
  return [
    ...(app === "codex" ? inventory.codex.map((record) => ({ kind: "codex" as const, record })) : []),
    ...inventory.profiles.filter(({ profile }) => profile.app === app)
      .map((record) => ({ kind: "profile" as const, record })),
  ];
}

export function routeRoles(document: RouteDocument): RouteRole[] {
  if (document.kind === "codex") return ["main", "subagent"];
  return document.draft.routeMode === "official" ? ["main"] : ["main", "haiku", "sonnet", "opus", "fable"];
}

export function roleTarget(document: RouteDocument, role: RouteRole, codex: CodexProviderRecord[]) {
  if (document.kind === "codex") {
    const route = role === "subagent" ? document.draft.subagentRoute : null;
    return role === "subagent" && !route ? null : {
      provider: route ? codex.find(({ profile }) => profile.id === route.profileId)?.profile.name ?? null : document.draft.name,
      model: route ? route.model : document.draft.defaultModel,
    };
  }
  const options = document.draft.modelOptions;
  const model = role === "main" ? document.draft.model
    : options?.kind === "claude" && role !== "subagent" ? options[`${role}Model`] : null;
  return role === "main" || model ? { provider: document.draft.name, model: model ?? "" } : null;
}

export function routeIssue(document: RouteDocument, codex: CodexProviderRecord[]): MessageKey | null {
  if (document.kind !== "codex") return null;
  if (!document.draft.catalog.some(({ id }) => id === document.draft.defaultModel)) return "modelRoutes.chooseMainModel";
  const route = document.draft.subagentRoute;
  if (!route) return null;
  if (!route.profileId || !route.model) return "modelRoutes.chooseTarget";
  const target = route.profileId === document.record.profile.id ? document.draft : codex.find(({ profile }) => profile.id === route.profileId)?.profile;
  if (!target) return "codex.subagent.missingTarget";
  if (target.connection?.authBinding) return "codex.subagent.authBoundTarget";
  return target.catalog.some(({ id }) => id === route.model) ? null : "codex.subagent.modelNotInCatalog";
}

export function resetRouting(document: RouteDocument): RouteDocument {
  if (document.kind === "codex") return { ...document, draft: { ...document.draft,
    defaultModel: document.record.profile.defaultModel, subagentRoute: null } };
  return { ...document, draft: { ...document.draft, model: null,
    modelOptions: document.draft.routeMode === "official" ? null : claudeOptions(document.draft.modelOptions, {
      primaryOneM: false, haikuModel: null, haikuOneM: false, sonnetModel: null, sonnetOneM: false,
      opusModel: null, opusOneM: false, fableModel: null, fableOneM: false,
    }) } };
}
