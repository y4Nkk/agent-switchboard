import { useCallback, useEffect, useRef, useState } from "react";
import { getTraySnapshot, onTrayChanged, type TraySnapshot } from "../api/client";

export function trayError(caught: unknown): string {
  if (typeof caught === "string") return caught;
  if (caught instanceof Error) return caught.message;
  if (caught && typeof caught === "object" && "message" in caught) return String(caught.message);
  return "托盘操作失败，请打开主界面检查。";
}

export function useTraySnapshot() {
  const [snapshot, setSnapshot] = useState<TraySnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [initialized, setInitialized] = useState(false);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const refresh = useCallback(() => refreshRef.current(), []);

  useEffect(() => {
    let disposed = false;
    let revision = 0;
    let unlisten: (() => void) | undefined;
    const reload = async () => {
      const request = ++revision;
      try {
        const next = await getTraySnapshot();
        if (!disposed && request === revision) { setSnapshot(next); setError(null); }
      } catch (caught) {
        if (!disposed && request === revision) setError(trayError(caught));
      }
    };
    refreshRef.current = reload;
    void (async () => {
      try {
        const stop = await onTrayChanged(() => { void reload(); });
        if (disposed) { stop(); return; }
        unlisten = stop;
        await reload();
      } catch (caught) {
        if (!disposed) setError(trayError(caught));
      } finally {
        if (!disposed) setInitialized(true);
      }
    })();
    return () => { disposed = true; revision++; unlisten?.(); refreshRef.current = async () => {}; };
  }, []);
  return { snapshot, error, initialized, refresh };
}
