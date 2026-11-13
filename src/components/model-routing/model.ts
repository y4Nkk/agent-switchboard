import type { AppKind, CodexProviderDraft, CodexProviderRecord, ProviderDraft, ProviderRecord, ClaudeModelRoute } from "../../api/client";
import { claudeOptions } from "../../lib/provider-drafts.ts";
import type { MessageKey } from "../../i18n";

export type RouteRole = "main" | "subagent" | "haiku" | "sonnet" | "opus" | "fable";
export type CodexRouteDraft = Pick<CodexProviderDraft, "name" | "defaultModel" | "connection" | "subagentRoute"> & {
  catalog: { id: string }[];
};
export type ClaudeRouteDraft = Pick<ProviderDraft, "name" | "routeMode" | "model" | "modelOptions" | "connection">;
export type RouteDocument =
  | { kind: "codex"; profileId: string; draft: CodexRouteDraft }
  | { kind: "profile"; profileId: string; draft: ClaudeRouteDraft };
export type RouteRecord =
  | { kind: "codex"; record: CodexProviderRecord }
  | { kind: "profile"; record: ProviderRecord };

export interface RouteInventory {
  codex: CodexProviderRecord[];
  profiles: ProviderRecord[];
  activeIds: Partial<Record<AppKind, string | null>>;
}

export function routeDocument(source: Extract<RouteRecord, { kind: "codex" }>): Extract<RouteDocument, { kind: "codex" }>;
export function routeDocument(source: Extract<RouteRecord, { kind: "profile" }>): Extract<RouteDocument, { kind: "profile" }>;
export function routeDocument(source: RouteRecord): RouteDocument;
export function routeDocument(source: RouteRecord): RouteDocument {
  const profile = source.record.profile;
  if (source.kind === "codex") {
    const codex = source.record.profile;
    return { kind: "codex", profileId: codex.id, draft: {
      name: codex.name, defaultModel: codex.defaultModel,
      connection: codex.connection, subagentRoute: codex.subagentRoute,
      catalog: codex.catalog.map(({ id }) => ({ id })),
    } };
  }
  const claude = source.record.profile;
  return { kind: "profile", profileId: profile.id, draft: {
    name: claude.name, routeMode: claude.routeMode, model: claude.model,
    modelOptions: claude.modelOptions, connection: claude.connection,
  } };
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
  return document.draft.routeMode === "official" ? ["main"] : ["main", "haiku", "sonnet", "opus", "fable", "subagent"];
}

export function claudeModels(profile: ClaudeRouteDraft, profileId?: string): string[] {
  const options = profile.modelOptions?.kind === "claude" ? profile.modelOptions : null;
  return [...new Set([profile.model, ...(options?.availableModels ?? []),
    ...Object.values(options?.routes ?? {}).filter((route) => route?.mode === "primary" || route?.mode === "provider" && route.profileId === profileId).map((route) => route!.model)])]
    .filter((model): model is string => !!model);
}

export function claudeTargetIssue(profile: ClaudeRouteDraft): MessageKey | null {
  if (profile.routeMode === "official" || profile.connection?.claudeNative || profile.connection?.authBinding)
    return "modelRoutes.unsupportedTarget";
  return null;
}

export function roleTarget(document: RouteDocument, role: RouteRole, inventory: RouteInventory) {
  if (document.kind === "codex") {
    const route = role === "subagent" ? document.draft.subagentRoute : null;
    return role === "subagent" && !route ? null : {
      provider: !route || route.profileId === document.profileId ? document.draft.name
        : inventory.codex.find(({ profile }) => profile.id === route.profileId)?.profile.name ?? null,
      model: route ? route.model : document.draft.defaultModel,
    };
  }
  if (role === "main") return { provider: document.draft.name, model: document.draft.model ?? "" };
  const options = document.draft.modelOptions;
  const route = options?.kind === "claude" ? options.routes[role] ?? (role === "fable" ? options.routes.opus : null) : null;
  if (!route) return null;
  return { provider: route.mode === "primary" || route.profileId === document.profileId ? document.draft.name
    : inventory.profiles.find(({ profile }) => profile.id === route.profileId)?.profile.name ?? null, model: route.model };
}

export function routeDescription(document: RouteDocument, role: RouteRole, inventory: RouteInventory) {
  const target = roleTarget(document, role, inventory);
  if (document.kind === "codex") return { target, mode: role === "subagent" && target ? "provider" : "primary", oneM: false };
  const options = document.draft.modelOptions?.kind === "claude" ? document.draft.modelOptions : null;
  const route = role === "main" ? null : options?.routes[role] ?? (role === "fable" ? options?.routes.opus : null);
  return { target, mode: route?.mode ?? "primary", oneM: role === "main" ? options?.primaryOneM ?? false : route?.oneM ?? false };
}

function claudeRouteIssue(document: Extract<RouteDocument, { kind: "profile" }>, route: ClaudeModelRoute,
  inventory: RouteInventory): MessageKey | null {
  if (!route.model.trim()) return "modelRoutes.chooseTarget";
  if (route.mode === "primary") return null;
  if (!route.profileId) return "modelRoutes.chooseTarget";
  const target = route.profileId === document.profileId ? document.draft
    : inventory.profiles.find(({ profile }) => profile.app === "claude" && profile.id === route.profileId)?.profile;
  if (!target) return "modelRoutes.missingTarget";
  const unsupported = claudeTargetIssue(target);
  if (unsupported) return unsupported;
  return claudeModels(target, route.profileId).includes(route.model) ? null : "modelRoutes.modelNotInTarget";
}

export function routeRoleIssue(document: RouteDocument, role: RouteRole, inventory: RouteInventory): MessageKey | null {
  if (document.kind === "profile") {
    if (role === "main" || document.draft.modelOptions?.kind !== "claude") return null;
    const route = document.draft.modelOptions.routes[role];
    return route ? claudeRouteIssue(document, route, inventory) : null;
  }
  if (role === "main") return document.draft.catalog.some(({ id }) => id === document.draft.defaultModel)
    ? null : "modelRoutes.chooseMainModel";
  const route = document.draft.subagentRoute;
  if (!route) return null;
  if (!route.profileId || !route.model) return "modelRoutes.chooseTarget";
  const target = route.profileId === document.profileId ? document.draft
    : inventory.codex.find(({ profile }) => profile.id === route.profileId)?.profile;
  if (!target) return "codex.subagent.missingTarget";
  if (target.connection?.authBinding) return "codex.subagent.authBoundTarget";
  return target.catalog.some(({ id }) => id === route.model) ? null : "codex.subagent.modelNotInCatalog";
}

export function routeIssueRole(document: RouteDocument, inventory: RouteInventory): RouteRole | null {
  return routeRoles(document).find((role) => routeRoleIssue(document, role, inventory)) ?? null;
}

export function routeIssue(document: RouteDocument, inventory: RouteInventory): MessageKey | null {
  const role = routeIssueRole(document, inventory);
  return role ? routeRoleIssue(document, role, inventory) : null;
}

export function resetRouting(document: RouteDocument): RouteDocument {
  if (document.kind === "codex") return { ...document, draft: { ...document.draft, subagentRoute: null } };
  return { ...document, draft: { ...document.draft,
    modelOptions: document.draft.routeMode === "official" ? null : claudeOptions(document.draft.modelOptions, {
      routes: { haiku: null, sonnet: null, opus: null, fable: null, subagent: null },
    }) } };
}
