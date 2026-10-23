import { useCallback, useEffect, useRef } from "react";
import { onCodexProbeChanged, type CodexProbeBatch } from "../api/client";
import { toast, toastMessage } from "../components/use-toast";
import { CommandErrorLines } from "./notifications";
import { advanceProbeNotification, type ProbeNotificationState } from "./probe-notifications";

export function notifyProbeError(stage: "read" | "watch" | "start" | "cancel" | "save", error: unknown) {
  const titles = { read: "codex.probe.notify.readFailed", watch: "codex.probe.notify.watchFailed",
    start: "codex.probe.notify.startFailed", cancel: "codex.probe.notify.cancelFailed",
    save: "codex.probe.notify.saveFailed" } as const;
  return toast({ kind: "error", durationMs: stage === "read" || stage === "watch" ? 0 : undefined,
    title: toastMessage(titles[stage]), description: <CommandErrorLines error={error} /> });
}

export function useProbeNotifications(status: CodexProbeBatch | null, synchronized: boolean) {
  const state = useRef<ProbeNotificationState>({ watching: null, delivered: null });
  const dismiss = useRef<(() => void) | null>(null);
  const watch = useCallback((batchId: string) => { state.current.watching = batchId; }, []);
  useEffect(() => {
    if (!synchronized) return;
    const next = advanceProbeNotification(state.current, status);
    state.current = next.state;
    if (next.notice) {
      dismiss.current?.();
      dismiss.current = toast(next.notice);
    }
  }, [status, synchronized]);
  return watch;
}

export function useProbeEvents(refresh: () => void, watch: (batchId: string) => void) {
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let dismissError: (() => void) | undefined;
    const subscribe = async () => {
      try {
        const unlisten = await onCodexProbeChanged((batchId) => {
          if (active) { watch(batchId); refresh(); }
        });
        if (!active) { unlisten(); return; }
        stop = unlisten;
        dismissError?.();
        dismissError = undefined;
        // Subscribe before this read so changes during the first read are recovered.
        refresh();
      } catch (error) {
        if (!active) return;
        if (!dismissError) dismissError = notifyProbeError("watch", error);
        retry = setTimeout(() => void subscribe(), 1000);
      }
    };
    const visible = () => { if (!document.hidden) refresh(); };
    void subscribe();
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", visible);
    return () => {
      active = false; stop?.(); clearTimeout(retry); dismissError?.();
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", visible);
    };
  }, [refresh, watch]);
}
