import { useCallback, useEffect, useRef, useState } from "react";
import { readResourceUsage, type ResourceGroup, type ResourceRead, type ResourceSelection } from "../api/client";
import { useReadQueue } from "../hooks/use-read-queue";
import { useMessageState } from "../i18n/use-message-state";
import { startResourceReading, type ResourceReadActivity } from "./resource-usage-reader";

function selectionKey(selection: ResourceSelection | null) {
  return selection ? `${selection.group}:${selection.page}` : "none";
}

export function useResourceUsage(active: boolean) {
  const [result, setResult] = useState<{ read: ResourceRead; selection: ResourceSelection | null; memoryMaximum: number | null } | null>(null);
  const [selection, setSelection] = useState<ResourceSelection | null>(null);
  const selectionRef = useRef(selection);
  const [activity, setActivity] = useState<ResourceReadActivity>(null);
  const [error, setError] = useMessageState();
  const enqueue = useReadQueue();
  const reader = useRef<ReturnType<typeof startResourceReading> | null>(null);
  useEffect(() => {
    if (!active) return;
    const lifetime = startResourceReading({ enqueue, read: readResourceUsage,
      selection: () => selectionRef.current, onActivity: setActivity, onError: setError,
      onRead: (read, selected, memoryMaximum) => {
        const effective = selected && read.details ? { group: read.details.group, page: read.details.page } : selected;
        if (selectionKey(effective) !== selectionKey(selected)) {
          selectionRef.current = effective;
          setSelection(effective);
        }
        setResult({ read, selection: effective, memoryMaximum });
      } });
    reader.current = lifetime;
    return () => { lifetime.stop(); reader.current = null; };
  }, [active, enqueue, setError]);
  const select = useCallback((next: ResourceSelection | null) => {
    selectionRef.current = next;
    setSelection(next);
    reader.current?.select();
  }, []);
  const toggle = useCallback((group: ResourceGroup) => {
    select(selectionRef.current?.group === group ? null : { group, page: 1 });
  }, [select]);
  const setPage = useCallback((page: number) => {
    const selected = selectionRef.current;
    if (selected) select({ group: selected.group, page });
  }, [select]);
  const refresh = useCallback(() => reader.current?.refresh(), []);
  const details = selection && result?.read.details?.group === selection.group ? result.read.details : null;
  const currentDetails = result !== null && selectionKey(result.selection) === selectionKey(selection);
  return { read: result?.read ?? null, details, memoryMaximum: result?.memoryMaximum ?? null,
    initialLoading: activity === "initial" && result === null,
    refreshing: activity === "manual",
    detailsLoading: selection !== null && activity !== null && (activity === "details" || !currentDetails),
    selection, toggle, setPage, refresh, error };
}
