import { useSyncExternalStore } from "react";
import { onWorkspaceVisibilityChanged } from "../api/client";

const listeners = new Set<() => void>();
const notify = () => listeners.forEach((listener) => listener());
let nativeVisible = false;
let nativeGeneration = 0;
let stopNative: (() => void) | undefined;

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (listeners.size === 1) {
    document.addEventListener("visibilitychange", notify);
    const generation = ++nativeGeneration;
    void onWorkspaceVisibilityChanged((visible) => {
      if (generation !== nativeGeneration || listeners.size === 0) return;
      if (nativeVisible !== visible) { nativeVisible = visible; notify(); }
    }).then((stop) => {
      if (generation === nativeGeneration && listeners.size > 0) stopNative = stop;
      else stop();
    }).catch((error: unknown) => console.error("Workspace visibility could not be read", error));
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      nativeGeneration += 1;
      stopNative?.();
      stopNative = undefined;
      document.removeEventListener("visibilitychange", notify);
    }
  };
}
const snapshot = () => document.visibilityState === "visible" && nativeVisible;

export function useWorkspaceVisible(active: boolean): boolean {
  const visible = useSyncExternalStore(subscribe, snapshot, () => true);
  return active && visible;
}
