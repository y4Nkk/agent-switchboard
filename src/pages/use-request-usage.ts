import { useCallback, useEffect, useRef, useState } from "react";
import { readRequestUsagePages, refreshRequestUsage, type RequestUsageQuery, type RequestUsageReport } from "../api/client";
import type { RequestUsageAnalysisSelection } from "../components/request-usage/RequestUsageAnalysis";
import { useI18n } from "../i18n";
import { errorText } from "../i18n/errors";

const INITIAL_QUERY: RequestUsageQuery = { range: "last24Hours", app: null, scope: "generation", model: null, page: 1, groupPage: 1 };
const INITIAL_ANALYSIS: RequestUsageAnalysisSelection = { view: "trend", trend: null, models: null, requests: null };
const REFRESH_INTERVAL = 30_000;

interface SavedReport {
  scope: string;
  revision: number;
  refreshedAt: number;
  page: number;
  groupPage: number;
  report: RequestUsageReport;
}

export function useRequestUsage(active: boolean) {
  const [query, setQuery] = useState(INITIAL_QUERY);
  const [model, setModel] = useState("");
  const [analysis, setAnalysis] = useState(INITIAL_ANALYSIS);
  const read = useRequestUsageReport(active, query);

  useEffect(() => {
    const filter = model.trim() || null;
    const timer = setTimeout(() => setQuery((current) => current.model === filter ? current
      : { ...current, model: filter, page: 1, groupPage: 1 }), 350);
    return () => clearTimeout(timer);
  }, [model]);

  const updateFilters = (patch: Partial<RequestUsageQuery>) => {
    if (patch.scope && patch.scope !== query.scope) setAnalysis((current) => ({ ...INITIAL_ANALYSIS, view: current.view }));
    setQuery((current) => ({ ...current, ...patch, page: 1, groupPage: 1 }));
  };
  const filtered = query.range !== "all" || query.app !== null || query.model !== null || model.trim() !== "";
  const showAll = filtered ? () => {
    setModel("");
    setQuery({ ...INITIAL_QUERY, scope: query.scope, range: "all" });
  } : undefined;

  return {
    ...read, active, query, model, setModel, updateFilters, analysis, setAnalysis, showAll,
    setPage: (page: number) => setQuery((current) => ({ ...current, page })),
    setGroupPage: (groupPage: number) => setQuery((current) => ({ ...current, groupPage })),
  };
}

/** One visible query owns polling. Late responses never replace a different filter's report. */
function useRequestUsageReport(active: boolean, query: RequestUsageQuery) {
  const { t } = useI18n();
  const key = JSON.stringify(query);
  const scope = JSON.stringify([query.range, query.app, query.scope, query.model]);
  const [saved, setSaved] = useState<SavedReport | null>(null);
  const [failure, setFailure] = useState<{ scope: string; error: unknown } | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const last = useRef<SavedReport | null>(null);
  const flight = useRef<Promise<void> | null>(null);

  useEffect(() => {
    if (!active) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const selected = JSON.parse(key) as RequestUsageQuery;
    const { page, groupPage } = selected;
    const schedule = (delay: number) => {
      timer = setTimeout(() => void read(true), Math.max(0, delay));
    };
    const read = async (periodic = false) => {
      while (flight.current) {
        await flight.current;
        if (stopped) return;
      }
      const current = last.current;
      const cached = current?.scope === scope && current.revision === revision ? current : null;
      const shouldRefresh = periodic || cached === null;
      if (!shouldRefresh && cached && cached.page === page && cached.groupPage === groupPage) {
        setSaved(cached);
        setBusyKey(null);
        schedule(cached.refreshedAt + REFRESH_INTERVAL - Date.now());
        return;
      }
      setBusyKey(key);
      const pending = (async () => {
        try {
          const report = !shouldRefresh && cached
            ? { ...cached.report, ...await readRequestUsagePages(cached.report.snapshotId, page, groupPage) }
            : await refreshRequestUsage(selected);
          const result = { scope, revision, page, groupPage, report,
            refreshedAt: shouldRefresh || !cached ? Date.now() : cached.refreshedAt };
          last.current = result;
          if (!stopped) {
            setSaved(result);
            setFailure(null);
            schedule(result.refreshedAt + REFRESH_INTERVAL - Date.now());
          }
        } catch (error) {
          if (!stopped) {
            setFailure({ scope, error });
            schedule(REFRESH_INTERVAL);
          }
        } finally {
          if (!stopped) setBusyKey(null);
        }
      })();
      flight.current = pending;
      await pending;
      if (flight.current === pending) flight.current = null;
    };
    void read();
    return () => { stopped = true; if (timer) clearTimeout(timer); };
  }, [active, key, scope, revision]);

  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  const report = saved?.scope === scope ? saved.report : null;
  const readError = failure?.scope === scope ? errorText(failure.error, t) : null;
  return { report, error: readError,
    loading: active && busyKey === key, refresh };
}
