import type { AppKind, CodexProviderRecord, ProviderProfile, UsageQuery } from "../api/client";

export interface QuotaProvider { app: AppKind; id: string; name: string; usageQuery: UsageQuery | null }

export function quotaProviders(
  claude: ReadonlyArray<{ profile: Pick<ProviderProfile, "app" | "id" | "name" | "routeMode" | "usageQuery"> }>,
  codex: ReadonlyArray<{ profile: Pick<CodexProviderRecord["profile"], "id" | "name">; usageQuery: UsageQuery | null }>,
): QuotaProvider[] {
  return [
    ...codex.map((record) => ({ app: "codex" as const, id: record.profile.id, name: record.profile.name, usageQuery: record.usageQuery })),
    ...claude.filter(({ profile }) => profile.app === "claude").map(({ profile }) => ({
      app: profile.app, id: profile.id, name: profile.name,
      usageQuery: profile.routeMode === "official" ? null : profile.usageQuery ?? null,
    })),
  ];
}

export async function refreshProviderQuotas(profiles: readonly QuotaProvider[],
  query: (app: AppKind, id: string) => Promise<unknown>): Promise<void> {
  const configured = profiles.filter((profile) => profile.usageQuery !== null);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(3, configured.length) }, async () => {
    while (next < configured.length) {
      const profile = configured[next++];
      try { await query(profile.app, profile.id); }
      catch { /* Each row reads its failure and retained values from the backend cache. */ }
    }
  }));
}
