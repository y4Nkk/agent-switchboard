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

function useLogSnapshot(enabled: boolean, filter: RuntimeLogFilter, leaving: boolean) {
  const [snapshot, setSnapshot] = useState<{ filter: RuntimeLogFilter; read: RuntimeLogRead } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{ filter: RuntimeLogFilter; problem: CommandError } | null>(null);
  const generation = useRef(0);
  const page = useRef(1);
  const visible = useRef(enabled);
  visible.current = enabled;
  const readPage = useCallback(async (requestedPage: number, snapshotId?: string) => {
    if (!visible.current) return;
    const current = ++generation.current;
    setLoading(true); setError(null);
    try {
      const result = await listRuntimeLogs(snapshotId === undefined
        ? { kind: "refresh", level: filter === "all" ? null : filter, page: requestedPage }
        : { kind: "page", snapshotId, page: requestedPage });
      if (current === generation.current) {
        page.current = result.page;
        setSnapshot({ filter, read: result });
      }
    } catch (caught) { if (current === generation.current) setError({ filter, problem: caught as CommandError }); }
    finally { if (current === generation.current) setLoading(false); }
  }, [filter]);
  useEffect(() => {
    if (enabled || !leaving) {
      setSnapshot(null);
      if (enabled) void readPage(page.current);
      else setLoading(false);
    }
    return () => { generation.current += 1; };
  }, [enabled, leaving, readPage]);
  const read = snapshot?.filter === filter ? snapshot.read : null;
  const resetPage = () => { generation.current += 1; page.current = 1; };
  return {
    read, loading, error: error?.filter === filter ? error.problem : null,
    goToPage: (next: number) => { if (read) void readPage(next, read.snapshotId); },
    resetPage, refresh: () => readPage(page.current),
  };
}

export function useRuntimeLogs(enabled: boolean, leaving: boolean) {
  const [filter, setFilter] = useState<RuntimeLogFilter>("all");
  const [openingFolder, setOpeningFolder] = useState(false);
  const [folderError, setFolderError] = useState<CommandError | null>(null);
  const snapshot = useLogSnapshot(enabled, filter, leaving);
  const openLogDirectory = useCallback(async () => {
    setOpeningFolder(true); setFolderError(null);
    try { await openRuntimeLogDir(); }
    catch (caught) { setFolderError(caught as CommandError); }
    finally { setOpeningFolder(false); }
  }, []);
  const applyFilter = (next: RuntimeLogFilter) => {
    if (next === filter) {
      if ((snapshot.read?.page ?? 1) !== 1) snapshot.goToPage(1);
    } else {
      snapshot.resetPage();
      setFilter(next);
    }
  };
  return { read: snapshot.read, loading: snapshot.loading, error: snapshot.error, refresh: snapshot.refresh,
    setPage: snapshot.goToPage,
    filter, setFilter: applyFilter, openingFolder, folderError, openLogDirectory };
}
