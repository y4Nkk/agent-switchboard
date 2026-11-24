import { useCallback, useEffect, useRef, useState } from "react";
import {
  listRuntimeLogs,
  openRuntimeLogDir,
  type CommandError,
  type RuntimeLogRead,
  type RuntimeLogSeverity,
} from "../api/client";
import { RUNTIME_LOG_LEVEL_LABELS, type RuntimeLogFilter } from "../lib/runtime-log-labels";
import { tr } from "../i18n/current";

export function levelLabel(level: RuntimeLogSeverity): string {
  return tr(RUNTIME_LOG_LEVEL_LABELS[level]);
}

/** The log table's page size, matching the codebase's page-size convention. */
export const RUNTIME_LOG_PAGE_SIZE = 20;


function useLogSnapshot(enabled: boolean, filter: RuntimeLogFilter, leaving: boolean) {
  const [snapshot, setSnapshot] = useState<{ filter: RuntimeLogFilter; read: RuntimeLogRead } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{ filter: RuntimeLogFilter; problem: CommandError } | null>(null);
  const generation = useRef(0);
  const visible = useRef(enabled);
  visible.current = enabled;
  const refresh = useCallback(async () => {
    if (!visible.current) return;
    const current = ++generation.current;
    setLoading(true); setError(null);
    try {
      const result = await listRuntimeLogs(filter === "all" ? null : filter);
      if (current === generation.current) {
        setSnapshot({ filter, read: result });
      }
    } catch (caught) { if (current === generation.current) setError({ filter, problem: caught as CommandError }); }
    finally { if (current === generation.current) setLoading(false); }
  }, [filter]);
  useEffect(() => {
    if (enabled || !leaving) {
      setSnapshot(null);
      if (enabled) void refresh();
      else setLoading(false);
    }
    return () => { generation.current += 1; };
  }, [enabled, leaving, refresh]);
  const read = snapshot?.filter === filter ? snapshot.read : null;
  return { entries: read?.entries ?? [], total: read?.total ?? 0, skipped: read?.skipped ?? 0,
    loading, error: error?.filter === filter ? error.problem : null, refresh };
}

export function useRuntimeLogs(enabled: boolean, leaving: boolean) {
  const [filter, setFilter] = useState<RuntimeLogFilter>("all");
  const [page, setPage] = useState(1);
  const [openingFolder, setOpeningFolder] = useState(false);
  const [folderError, setFolderError] = useState<CommandError | null>(null);
  const snapshot = useLogSnapshot(enabled, filter, leaving);
  const openLogDirectory = useCallback(async () => {
    setOpeningFolder(true); setFolderError(null);
    try { await openRuntimeLogDir(); }
    catch (caught) { setFolderError(caught as CommandError); }
    finally { setOpeningFolder(false); }
  }, []);
  const applyFilter = useCallback((next: RuntimeLogFilter) => { setFilter(next); setPage(1); }, []);
  const pageCount = Math.max(1, Math.ceil(snapshot.entries.length / RUNTIME_LOG_PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const pageEntries = snapshot.entries.slice((currentPage - 1) * RUNTIME_LOG_PAGE_SIZE, currentPage * RUNTIME_LOG_PAGE_SIZE);
  return { ...snapshot, visibleEntries: snapshot.entries, pageEntries, page: currentPage, setPage,
    filter, setFilter: applyFilter, openingFolder, folderError, openLogDirectory };
}
