import { uiMessage } from "../i18n/errors";
import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useState } from "react";
import { toast, toastMessage } from "./use-toast";
import {
  deleteCodexProbeBatches, getCodexProbeBatch, listCodexProbeHistory,
  listCodexProbeHistoryProfiles, type CodexProbeBatch, type CodexProbeBatchStatus,
  type CodexProbeHistoryItem, type CodexProbeHistoryRange,
} from "../api/client";

const PAGE_SIZE = 20;
export interface CodexProbeHistoryProfileOption {
  profileId: string;
  profileName: string;
}

function useHistoryFilters() {
  const [range, setRange] = useState<CodexProbeHistoryRange>("all");
  const [profileFilter, setProfileFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [page, setPage] = useState(1);
  const changeRange = useCallback((value: CodexProbeHistoryRange) => { setRange(value); setPage(1); }, []);
  const changeProfile = useCallback((value: string) => { setProfileFilter(value); setPage(1); }, []);
  const changeStatus = useCallback((value: string) => { setStatusFilter(value); setPage(1); }, []);
  const clearFilters = useCallback(() => {
    setRange("all"); setProfileFilter("all"); setStatusFilter("all"); setPage(1);
  }, []);
  return { range, changeRange, profileFilter, changeProfile, statusFilter, changeStatus,
    page, setPage, clearFilters };
}

function useHistoryPage(enabled: boolean, revision: string, epoch: number,
  filters: ReturnType<typeof useHistoryFilters>) {
  const { range, profileFilter, statusFilter, page, setPage } = filters;
  const [items, setItems] = useState<CodexProbeHistoryItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useMessageState();
  const [selection, setSelection] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    setLoading(true);
    setListError(null);
    listCodexProbeHistory({ offset: (page - 1) * PAGE_SIZE, limit: PAGE_SIZE, range,
      profile: profileFilter === "all" ? { kind: "all" }
        : profileFilter === "unlinked" ? { kind: "unlinked" } : { kind: "profile", id: profileFilter },
      status: statusFilter as CodexProbeBatchStatus | "all",
    }).then((result) => {
      if (!active) return;
      setTotal(result.total);
      setItems(result.items);
      const lastPage = Math.max(1, Math.ceil(result.total / PAGE_SIZE));
      if (page > lastPage) setPage(lastPage);
      const selectable = new Set(result.items.filter((item) => item.status !== "running").map((item) => item.batchId));
      setSelection((previous) => new Set([...previous].filter((id) => selectable.has(id))));
    }).catch((reason) => {
      if (active) setListError(reason);
    }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [enabled, revision, epoch, range, profileFilter, statusFilter, page, setPage]);
  const toggleSelected = useCallback((id: string, checked: boolean) => {
    setSelection((previous) => {
      const next = new Set(previous);
      if (checked) next.add(id); else next.delete(id);
      return next;
    });
  }, []);
  const clearSelection = useCallback(() => setSelection(new Set()), []);
  return { items, total, loading, listError, selection, toggleSelected, clearSelection };
}

function useHistoryProfiles(enabled: boolean, revision: string, epoch: number) {
  const [profiles, setProfiles] = useState<CodexProbeHistoryProfileOption[] | null>(null);
  const [profilesError, setProfilesError] = useMessageState();
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    listCodexProbeHistoryProfiles().then((options) => {
      if (active) { setProfiles(options); setProfilesError(null); }
    }).catch((reason) => { if (active) setProfilesError(reason); });
    return () => { active = false; };
  }, [enabled, revision, epoch]);
  return { profiles, profilesError };
}

function useHistoryDetail(enabled: boolean, revision: string) {
  const [detailId, setDetailId] = useState<string | null>(null);
  const [detail, setDetail] = useState<CodexProbeBatch | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useMessageState();
  const [epoch, setEpoch] = useState(0);
  useEffect(() => {
    if (!enabled || detailId === null) return;
    let active = true;
    setDetailLoading(true);
    setDetailError(null);
    getCodexProbeBatch(detailId).then((batch) => {
      if (!active) return;
      setDetail(batch);
      setDetailError(batch === null ? uiMessage("codex.history.missingBatch") : null);
    }).catch((reason) => { if (active) setDetailError(reason); })
      .finally(() => { if (active) setDetailLoading(false); });
    return () => { active = false; };
  }, [enabled, detailId, epoch, revision]);
  const openDetail = useCallback((id: string) => {
    setDetail(null); setDetailError(null); setDetailLoading(true);
    setDetailId(id); setEpoch((value) => value + 1);
  }, []);
  const closeDetail = useCallback(() => {
    setDetailId(null); setDetail(null); setDetailLoading(false); setDetailError(null);
  }, []);
  return { detailId, detail, detailLoading, detailError, openDetail, closeDetail };
}

/** Visibility and live-batch revisions invalidate reads, retaining only UI
 * filters and selection across navigation. Obsolete requests cannot win. */
export function useCodexProbeHistory(enabled: boolean, revision: string) {
  const filters = useHistoryFilters();
  const [epoch, setEpoch] = useState(0);
  const page = useHistoryPage(enabled, revision, epoch, filters);
  const profiles = useHistoryProfiles(enabled, revision, epoch);
  const detail = useHistoryDetail(enabled, revision);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useMessageState();
  const refresh = useCallback(() => setEpoch((value) => value + 1), []);
  const { setPage } = filters;
  const { clearSelection } = page;
  const deleteSelected = useCallback(async (ids: string[]): Promise<boolean> => {
    setDeleting(true); setDeleteError(null);
    try {
      const result = await deleteCodexProbeBatches(ids);
      if (result.warning) toast({ kind: "warning", title: toastMessage("usage.radar.reclaimWarning", { detail: result.warning }) });
      clearSelection(); setPage(1); refresh();
      return true;
    } catch (reason) {
      setDeleteError(reason);
      return false;
    } finally { setDeleting(false); }
  }, [clearSelection, setPage, refresh]);
  return { pageSize: PAGE_SIZE, ...filters, ...page, ...profiles, ...detail,
    deleting, deleteError, deleteSelected, refresh };
}
