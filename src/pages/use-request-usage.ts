import { useCallback, useEffect, useRef, useState } from "react";
import { readRequestUsage, type RequestUsageQuery, type RequestUsageReport } from "../api/client";
import { useI18n } from "../i18n";
import { errorText } from "../i18n/errors";
import { clientName } from "../lib/client-name";

/** One visible query owns polling. Late responses never replace a different filter's report. */
export function useRequestUsage(active: boolean, query: RequestUsageQuery, refreshToken: number) {
  const { t } = useI18n();
  const key = JSON.stringify(query);
  const scope = JSON.stringify([query.range, query.app, query.profileId, query.model]);
  const [saved, setSaved] = useState<{ scope: string; report: RequestUsageReport } | null>(null);
  const [failure, setFailure] = useState<{ scope: string; error: unknown } | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const flight = useRef<{ key: string; promise: Promise<RequestUsageReport> } | null>(null);

  useEffect(() => {
    if (!active) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const read = async () => {
      setBusyKey(key);
      if (flight.current && flight.current.key !== key) {
        await flight.current.promise.catch(() => {});
        if (stopped) return;
      }
      let pending = flight.current?.key === key ? flight.current.promise : null;
      if (!pending) {
        pending = readRequestUsage(JSON.parse(key) as RequestUsageQuery);
        const current = { key, promise: pending };
        flight.current = current;
        const release = () => { if (flight.current === current) flight.current = null; };
        void pending.then(release, release);
      }
      try {
        const report = await pending;
        if (!stopped) {
          const issues = sourceIssues(report);
          setSaved((previous) => issues.length > 0 && previous?.scope === scope
            && previous.report.sources.every((source) => source.error === null) ? previous : { scope, report });
          setFailure(issues.length > 0 ? { scope, error: issues.join(" · ") } : null);
        }
      } catch (error) {
        if (!stopped) setFailure({ scope, error });
      } finally {
        if (!stopped) { setBusyKey(null); timer = setTimeout(() => void read(), 30_000); }
      }
    };
    void read();
    return () => { stopped = true; if (timer) clearTimeout(timer); };
  }, [active, key, scope, refreshToken, revision]);

  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  const report = saved?.scope === scope ? saved.report : null;
  const readError = failure?.scope === scope ? errorText(failure.error, t) : null;
  return { report, error: readError ?? (report ? sourceIssues(report).join(" · ") || null : null),
    loading: active && busyKey === key, refresh };
}

function sourceIssues(report: RequestUsageReport): string[] {
  return report.sources.flatMap((source) => source.error ? [`${clientName(source.app)}: ${source.error}`] : []);
}
