import type { AppKind, CodexProviderDraft, CodexProviderRecord, ProviderDraft, ProviderRecord, ClaudeModelRoute } from "../../api/client";
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
  return document.draft.routeMode === "official" ? ["main"] : ["main", "haiku", "sonnet", "opus", "fable", "subagent"];
}

export function claudeModels(profile: ProviderDraft, profileId?: string): string[] {
  const options = profile.modelOptions?.kind === "claude" ? profile.modelOptions : null;
  return [...new Set([profile.model, ...(options?.availableModels ?? []),
    ...Object.values(options?.routes ?? {}).filter((route) => route?.mode === "primary" || route?.mode === "provider" && route.profileId === profileId).map((route) => route!.model)])]
    .filter((model): model is string => !!model);
}

export function claudeTargetIssue(profile: ProviderDraft): MessageKey | null {
  if (profile.routeMode === "official" || profile.connection?.claudeNative || profile.connection?.authBinding)
    return "modelRoutes.unsupportedTarget";
  return null;
}

export function roleTarget(document: RouteDocument, role: RouteRole, inventory: RouteInventory) {
  if (document.kind === "codex") {
    const route = role === "subagent" ? document.draft.subagentRoute : null;
    return role === "subagent" && !route ? null : {
      provider: route ? inventory.codex.find(({ profile }) => profile.id === route.profileId)?.profile.name ?? null : document.draft.name,
      model: route ? route.model : document.draft.defaultModel,
    };
  }
  if (role === "main") return { provider: document.draft.name, model: document.draft.model ?? "" };
  const options = document.draft.modelOptions;
  const route = options?.kind === "claude" ? options.routes[role] ?? (role === "fable" ? options.routes.opus : null) : null;
  if (!route) return null;
  return { provider: route.mode === "primary" ? document.draft.name
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
  const target = route.profileId === document.record.profile.id ? document.draft
    : inventory.profiles.find(({ profile }) => profile.app === "claude" && profile.id === route.profileId)?.profile;
  if (!target) return "modelRoutes.missingTarget";
  const unsupported = claudeTargetIssue(target);
  if (unsupported) return unsupported;
  return claudeModels(target, route.profileId).includes(route.model) ? null : "modelRoutes.modelNotInTarget";
}

export function routeIssue(document: RouteDocument, inventory: RouteInventory): MessageKey | null {
  if (document.kind !== "codex") {
    if (document.draft.modelOptions?.kind !== "claude") return null;
    for (const route of Object.values(document.draft.modelOptions.routes)) {
      if (!route) continue;
      const issue = claudeRouteIssue(document, route, inventory);
      if (issue) return issue;
    }
    return null;
  }
  if (!document.draft.catalog.some(({ id }) => id === document.draft.defaultModel)) return "modelRoutes.chooseMainModel";
  const route = document.draft.subagentRoute;
  if (!route) return null;
  if (!route.profileId || !route.model) return "modelRoutes.chooseTarget";
  const target = route.profileId === document.record.profile.id ? document.draft
    : inventory.codex.find(({ profile }) => profile.id === route.profileId)?.profile;
  if (!target) return "codex.subagent.missingTarget";
  if (target.connection?.authBinding) return "codex.subagent.authBoundTarget";
  return target.catalog.some(({ id }) => id === route.model) ? null : "codex.subagent.modelNotInCatalog";
}

export function resetRouting(document: RouteDocument): RouteDocument {
  if (document.kind === "codex") return { ...document, draft: { ...document.draft,
    defaultModel: document.record.profile.defaultModel, subagentRoute: null } };
  return { ...document, draft: { ...document.draft, model: null,
    modelOptions: document.draft.routeMode === "official" ? null : claudeOptions(document.draft.modelOptions, {
      primaryOneM: false, routes: { haiku: null, sonnet: null, opus: null, fable: null, subagent: null },
    }) } };
}
