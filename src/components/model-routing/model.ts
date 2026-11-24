import type { AppKind, CodexProviderDraft, CodexProviderRecord, ProviderDraft, ProviderRecord, ClaudeModelRoute } from "../../api/client";
import { claudeOptions } from "../../lib/provider-drafts.ts";
import type { MessageKey } from "../../i18n";

export type RouteRole = "main" | "subagent" | "haiku" | "sonnet" | "opus" | "fable";
export type RouteCardStatus = "assigned" | "automatic" | "needsAttention";
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
  const route = effectiveClaudeRoute(document, role);
  if (!route) return null;
  return { provider: route.mode === "primary" || route.profileId === document.profileId ? document.draft.name
    : inventory.profiles.find(({ profile }) => profile.app === "claude" && profile.id === route.profileId)?.profile.name ?? null,
    model: route.model };
}

export function routeDescription(document: RouteDocument, role: RouteRole, inventory: RouteInventory): {
  target: ReturnType<typeof roleTarget>; mode: ClaudeModelRoute["mode"]; oneM: boolean;
} {
  const target = roleTarget(document, role, inventory);
  if (document.kind === "codex") return { target, mode: role === "subagent" && target ? "provider" : "primary", oneM: false };
  const options = document.draft.modelOptions?.kind === "claude" ? document.draft.modelOptions : null;
  const route = role === "main" ? null : effectiveClaudeRoute(document, role);
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
    if (role === "main") return null;
    const route = effectiveClaudeRoute(document, role);
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

export function routeCardState(document: RouteDocument, role: RouteRole, inventory: RouteInventory) {
  const { target, mode, oneM } = routeDescription(document, role, inventory);
  const issue = routeRoleIssue(document, role, inventory);
  const model = target?.model || null;
  const status: RouteCardStatus = issue ? "needsAttention" : model ? "assigned" : "automatic";
  const inherited = document.kind === "profile" && role === "fable" && !!target && !roleRoute(document, role);
  return { model, target, status, issue, inherited, oneM, rule: role === "main" ? "main" as const : mode };
}

export function resetRouting(document: RouteDocument): RouteDocument {
  if (document.kind === "codex") return { ...document, draft: { ...document.draft, subagentRoute: null } };
  return { ...document, draft: { ...document.draft,
    modelOptions: document.draft.routeMode === "official" ? null : claudeOptions(document.draft.modelOptions, {
      routes: { haiku: null, sonnet: null, opus: null, fable: null, subagent: null },
    }) } };
}

/** Where a purpose draws its models from: the main connection or one saved profile. */
export type RouteSource = "primary" | { kind: "provider"; profileId: string };
type PurposeRole = Exclude<RouteRole, "main">;
type ProfileDocument = Extract<RouteDocument, { kind: "profile" }>;
export type PurposeRoute = ClaudeModelRoute | NonNullable<CodexRouteDraft["subagentRoute"]>;

function claudeRoute(document: ProfileDocument, role: PurposeRole): ClaudeModelRoute | null {
  const options = document.draft.modelOptions;
  return options?.kind === "claude" ? options.routes[role] : null;
}

function effectiveClaudeRoute(document: ProfileDocument, role: PurposeRole): ClaudeModelRoute | null {
  return claudeRoute(document, role) ?? (role === "fable" ? claudeRoute(document, "opus") : null);
}

/** The purpose's own route; null while automatic, and always for the main model. */
export function roleRoute(document: RouteDocument, role: RouteRole): PurposeRoute | null {
  if (document.kind === "codex") return role === "subagent" ? document.draft.subagentRoute : null;
  return role === "main" ? null : claudeRoute(document, role);
}

export function roleSource(document: RouteDocument, role: RouteRole): RouteSource | null {
  const route = roleRoute(document, role);
  if (!route) return null;
  return "mode" in route && route.mode === "primary" ? "primary" : { kind: "provider", profileId: route.profileId };
}

export function sourceKey(source: RouteSource): string {
  return source === "primary" ? "primary" : `provider:${source.profileId}`;
}

/** Sources a purpose can be pointed at, in display order. Official, account-bound and
 * native cloud SDK connections never carry another provider's requests. */
export function roleSources(document: RouteDocument, inventory: RouteInventory): RouteSource[] {
  if (document.kind === "codex") {
    return [
      ...(!document.draft.connection?.authBinding ? [{ kind: "provider" as const, profileId: document.profileId }] : []),
      ...inventory.codex.filter(({ profile }) => profile.id !== document.profileId && !profile.connection?.authBinding)
        .map(({ profile }) => ({ kind: "provider" as const, profileId: profile.id })),
    ];
  }
  const native = !!document.draft.connection?.claudeNative;
  return [
    "primary",
    ...(!native && !claudeTargetIssue(document.draft) ? [{ kind: "provider" as const, profileId: document.profileId }] : []),
    ...(!native ? inventory.profiles
      .filter(({ profile }) => profile.app === "claude" && profile.id !== document.profileId && !claudeTargetIssue(profile))
      .map(({ profile }) => ({ kind: "provider" as const, profileId: profile.id })) : []),
  ];
}

export function sourceName(document: RouteDocument, source: RouteSource, inventory: RouteInventory): string | null {
  if (source === "primary" || source.profileId === document.profileId) return document.draft.name;
  const name = document.kind === "codex"
    ? inventory.codex.find(({ profile }) => profile.id === source.profileId)?.profile.name
    : inventory.profiles.find(({ profile }) => profile.app === "claude" && profile.id === source.profileId)?.profile.name;
  return name ?? null;
}

/** Models a source offers to a purpose: the draft's own catalog, or a saved profile's. */
export function sourceModels(document: RouteDocument, source: RouteSource, inventory: RouteInventory): string[] {
  if (document.kind === "codex") {
    const target = source === "primary" ? null : source.profileId === document.profileId ? document.draft
      : inventory.codex.find(({ profile }) => profile.id === source.profileId)?.profile;
    return target?.catalog.map(({ id }) => id) ?? [];
  }
  if (source === "primary" || source.profileId === document.profileId) return claudeModels(document.draft, document.profileId);
  const target = inventory.profiles.find(({ profile }) => profile.app === "claude" && profile.id === source.profileId)?.profile;
  return target ? claudeModels(target, source.profileId) : [];
}

function withClaudeRoute(document: ProfileDocument, role: PurposeRole, route: ClaudeModelRoute | null): ProfileDocument {
  const options = claudeOptions(document.draft.modelOptions, {});
  if (options.kind !== "claude") throw new Error("Claude routing requires Claude model options");
  return { ...document, draft: { ...document.draft,
    modelOptions: { ...options, routes: { ...options.routes, [role]: route } } } };
}

/** Points the purpose at a model of a source. The 1M flag survives a model change
 * and resets only when no model is chosen. The main model belongs to the catalog. */
export function setRoleModel(document: RouteDocument, role: RouteRole, source: RouteSource, model: string): RouteDocument {
  if (role === "main") return document;
  if (document.kind === "codex") return source === "primary" ? document
    : { ...document, draft: { ...document.draft, subagentRoute: { profileId: source.profileId, model } } };
  const oneM = model ? claudeRoute(document, role)?.oneM ?? false : false;
  return withClaudeRoute(document, role, source === "primary" ? { mode: "primary", model, oneM }
    : { mode: "provider", profileId: source.profileId, model, oneM });
}

/** Moves a routed purpose to another source, keeping its model when that source has it. */
export function setRoleSource(document: RouteDocument, role: RouteRole, source: RouteSource, inventory: RouteInventory): RouteDocument {
  const route = roleRoute(document, role);
  if (!route) return document;
  return setRoleModel(document, role, source,
    route.model && sourceModels(document, source, inventory).includes(route.model) ? route.model : "");
}

export function setRoleOneM(document: RouteDocument, role: RouteRole, oneM: boolean): RouteDocument {
  if (document.kind === "codex") return document;
  if (role === "main") return { ...document, draft: { ...document.draft,
    modelOptions: claudeOptions(document.draft.modelOptions, { primaryOneM: oneM }) } };
  const route = claudeRoute(document, role);
  return route ? withClaudeRoute(document, role, { ...route, oneM }) : document;
}

/** Returns the purpose to the client's own default rule. */
export function clearRoleRoute(document: RouteDocument, role: RouteRole): RouteDocument {
  if (role === "main") return document;
  return document.kind === "codex" ? { ...document, draft: { ...document.draft, subagentRoute: null } }
    : withClaudeRoute(document, role, null);
}
