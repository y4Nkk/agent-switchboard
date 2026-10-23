import { useCallback, useEffect, useRef, useState } from "react";
import { readResourceUsage, type ResourceGroup, type ResourceRead, type ResourceSelection } from "../api/client";
import { useReadQueue } from "../hooks/use-read-queue";
import { useMessageState } from "../i18n/use-message-state";
import { startResourceReading } from "./resource-usage-reader";

function selectionKey(selection: ResourceSelection | null) {
  return selection ? `${selection.group}:${selection.page}` : "none";
}

export function useResourceUsage(active: boolean) {
  const [result, setResult] = useState<{ read: ResourceRead; selection: ResourceSelection | null } | null>(null);
  const [selection, setSelection] = useState<ResourceSelection | null>(null);
  const selectionRef = useRef(selection);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useMessageState();
  const enqueue = useReadQueue();
  const reader = useRef<ReturnType<typeof startResourceReading> | null>(null);
  useEffect(() => {
    if (!active) { setLoading(false); return; }
    const lifetime = startResourceReading({ enqueue, read: readResourceUsage,
      selection: () => selectionRef.current, onLoading: setLoading, onError: setError,
      onRead: (read, selected) => {
        const effective = selected && read.details ? { group: read.details.group, page: read.details.page } : selected;
        if (selectionKey(effective) !== selectionKey(selected)) {
          selectionRef.current = effective;
          setSelection(effective);
        }
        setResult({ read, selection: effective });
      } });
    reader.current = lifetime;
    return () => { lifetime.stop(); reader.current = null; };
  }, [active, enqueue, setError]);
  const select = useCallback((next: ResourceSelection | null) => {
    selectionRef.current = next;
    setSelection(next);
    reader.current?.refresh();
  }, []);
  const toggle = useCallback((group: ResourceGroup) => {
    select(selectionRef.current?.group === group ? null : { group, page: 1 });
  }, [select]);
  const setPage = useCallback((page: number) => {
    const selected = selectionRef.current;
    if (selected) select({ group: selected.group, page });
  }, [select]);
  const refresh = useCallback(() => reader.current?.refresh(), []);
  return { read: result?.read ?? null,
    details: result && selectionKey(result.selection) === selectionKey(selection) ? result.read.details : null,
    selection, toggle, setPage, refresh, loading, error };
}
