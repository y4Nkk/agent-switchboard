import { useEffect, useMemo, useRef, useState } from "react";
import { onTrayChanged, queryProfileUsage, readProfileUsages, type ProfileUsageRead } from "../api/client";
import type { QuotaProvider } from "../lib/quota-profiles";
import { useI18n } from "../i18n";
import { errorText } from "../i18n/errors";

type QuotaIdentity = Pick<QuotaProvider, "app" | "id">;
type QuotaRead = { key: string; rows: Map<string, ProfileUsageRead> };
type QueryState = { key: string; error?: unknown };

export function quotaKey(profile: QuotaIdentity) {
  return `${profile.app}:${profile.id}`;
}

export function useProviderQuotaRead(active: boolean, profiles: readonly QuotaProvider[]) {
  const { t } = useI18n();
  const key = useMemo(() => JSON.stringify(profiles.filter((profile) => profile.usageQuery !== null)
    .map(({ app, id, usageQuery }) => ({ app, id, usageQuery }))), [profiles]);
  const [read, setRead] = useState<QuotaRead | null>(null);
  const lastRead = useRef<QuotaRead | null>(null);
  const [failure, setFailure] = useState<{ key: string; error: unknown } | null>(null);
  const [queries, setQueries] = useState<ReadonlyMap<string, QueryState>>(new Map());
  const running = useRef(new Set<string>());
  const reload = useRef<(() => Promise<void>) | null>(null);

  useEffect(() => {
    if (!active || key === "[]") return;
    const profiles = JSON.parse(key) as QuotaIdentity[];
    let disposed = false;
    let queued = false;
    let pending: Promise<void> | null = null;
    let unlisten: (() => void) | undefined;
    const request = profiles.map((profile) => ({ target: profile.app, profileId: profile.id }));
    const refresh = () => {
      queued = true;
      if (pending) return pending;
      pending = (async () => {
        do {
          queued = false;
          try {
            const rows = await readProfileUsages(request);
            if (disposed) return;
            if (rows.length !== profiles.length) throw new Error("Incomplete provider quota read");
            const next = { key, rows: new Map(profiles.map((profile, index) => [quotaKey(profile), rows[index]])) };
            const previous = lastRead.current;
            lastRead.current = next;
            setRead(next);
            setFailure(null);
            setQueries((current) => new Map([...current].filter(([id, query]) => query.key === key
              && (running.current.has(id) || (previous?.key === key
                && JSON.stringify(previous.rows.get(id)) === JSON.stringify(next.rows.get(id)))))));
          } catch (error) { if (!disposed) setFailure({ key, error }); }
        } while (!disposed && queued);
      })().finally(() => { pending = null; });
      return pending;
    };
    reload.current = refresh;
    void onTrayChanged(() => { void refresh(); }).then((stop) => {
      if (disposed) { stop(); return; }
      unlisten = stop;
      void refresh();
    }, () => { if (!disposed) void refresh(); });
    return () => {
      disposed = true;
      unlisten?.();
      reload.current = null;
    };
  }, [active, key]);

  const run = async (profile: QuotaProvider) => {
    const id = quotaKey(profile);
    if (running.current.has(id)) return;
    running.current.add(id);
    setQueries((previous) => new Map(previous).set(id, { key }));
    let error: unknown;
    try { await queryProfileUsage(profile.app, profile.id); }
    catch (reason) { error = reason; }
    await reload.current?.();
    running.current.delete(id);
    setQueries((previous) => {
      const next = new Map(previous);
      if (error === undefined) next.delete(id);
      else next.set(id, { key, error });
      return next;
    });
  };
  const rows = read?.key === key ? read.rows : null;
  return { ready: key === "[]" || rows !== null, row(profile: QuotaProvider) {
    const id = quotaKey(profile);
    const reading = rows?.get(id);
    const action = queries.get(id);
    const error = (action?.key === key ? action.error : null) ?? reading?.error ?? (failure?.key === key ? failure.error : null);
    return { summary: reading?.snapshot?.summary, error: error ? errorText(error, t) : reading?.snapshot?.error ?? null,
      querying: running.current.has(id), run: () => run(profile) };
  } };
}
