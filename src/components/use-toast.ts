import * as React from "react";

export type ToastKind = "info" | "success" | "warning" | "error";

export interface Toast {
  id: string;
  title?: React.ReactNode;
  description?: React.ReactNode;
  kind: ToastKind;
  /**
   * 自动关闭时间（毫秒）。
   * - 不传：使用默认值
   * - 0：不自动关闭（手动关闭）
   */
  durationMs?: number;
}

const TOAST_LIMIT = 2;
const DEFAULT_TOAST_REMOVE_DELAY: Record<ToastKind, number> = {
  info: 4500,
  success: 3200,
  warning: 6000,
  error: 10000,
};

let count = 0;

function genId() {
  count = (count + 1) % Number.MAX_SAFE_INTEGER;
  return count.toString();
}

type Action =
  | {
      type: "ADD_TOAST";
      toast: Toast;
    }
  | {
      type: "REMOVE_TOAST";
      toastId: Toast["id"];
    };

interface State {
  toasts: Toast[];
}

const listeners: Array<(state: State) => void> = [];
const toastTimers = new Map<string, ReturnType<typeof setTimeout>>();
const toastDeadlines = new Map<string, number>();
const pausedToastRemaining = new Map<string, number>();
let memoryState: State = { toasts: [] };

function clearToastTimer(toastId: string): void {
  const timer = toastTimers.get(toastId);
  if (timer !== undefined) clearTimeout(timer);
  toastTimers.delete(toastId);
  toastDeadlines.delete(toastId);
  pausedToastRemaining.delete(toastId);
}

function scheduleTimer(toastId: string, delayMs: number): void {
  clearToastTimer(toastId);
  toastDeadlines.set(toastId, Date.now() + delayMs);
  const timeout = setTimeout(() => {
    toastDeadlines.delete(toastId);
    pausedToastRemaining.delete(toastId);
    dismissToast(toastId);
  }, delayMs);
  toastTimers.set(toastId, timeout);
}

function scheduleAutoDismiss(toastId: string, kind: ToastKind, durationMs?: number): void {
  if (durationMs === 0) return;

  const delayMs = durationMs ?? DEFAULT_TOAST_REMOVE_DELAY[kind];
  if (delayMs === 0) return;
  scheduleTimer(toastId, delayMs);
}

function pauseAutoDismiss(toastId: string): void {
  const deadline = toastDeadlines.get(toastId);
  if (deadline === undefined) return;
  const remaining = Math.max(0, deadline - Date.now());
  clearToastTimer(toastId);
  pausedToastRemaining.set(toastId, remaining);
}

function resumeAutoDismiss(toastId: string): void {
  const remaining = pausedToastRemaining.get(toastId);
  if (remaining === undefined) return;
  if (remaining > 0) {
    scheduleTimer(toastId, remaining);
    return;
  }
  pausedToastRemaining.delete(toastId);
  dismissToast(toastId);
}

function dismissToast(toastId: string): void {
  if (!memoryState.toasts.some((toast) => toast.id === toastId)) return;

  clearToastTimer(toastId);
  dispatch({ type: "REMOVE_TOAST", toastId });
}

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "ADD_TOAST": {
      const toasts = [action.toast, ...state.toasts]
        .sort((left, right) => Number(right.kind === "error") - Number(left.kind === "error"))
        .slice(0, TOAST_LIMIT);
      return { toasts };
    }

    case "REMOVE_TOAST": {
      return {
        toasts: state.toasts.filter((toast) => toast.id !== action.toastId),
      };
    }
  }
}

function dispatch(action: Action) {
  memoryState = reducer(memoryState, action);
  listeners.forEach((listener) => {
    listener(memoryState);
  });
}

type ToastOptions = Omit<Toast, "id" | "kind"> & {
  kind: ToastKind;
};

function hasToastContent(value: React.ReactNode): boolean {
  if (value === null || value === undefined || typeof value === "boolean") return false;
  if (Array.isArray(value)) return value.some(hasToastContent);
  return typeof value !== "string" || value.trim().length > 0;
}

function toast(props: ToastOptions) {
  if (!hasToastContent(props.title) && !hasToastContent(props.description)) {
    throw new TypeError("Toast requires a title or description");
  }
  if (
    props.durationMs !== undefined &&
    (!Number.isFinite(props.durationMs) || props.durationMs < 0)
  ) {
    throw new RangeError("durationMs must be a finite non-negative number");
  }

  const id = genId();
  dispatch({
    type: "ADD_TOAST",
    toast: {
      ...props,
      id,
      kind: props.kind,
    },
  });
  const retainedIds = new Set(memoryState.toasts.map((item) => item.id));
  for (const toastId of toastTimers.keys()) {
    if (!retainedIds.has(toastId)) clearToastTimer(toastId);
  }
  if (retainedIds.has(id)) scheduleAutoDismiss(id, props.kind, props.durationMs);
}

function useToast() {
  const [state, setState] = React.useState<State>(memoryState);

  React.useEffect(() => {
    listeners.push(setState);
    return () => {
      const index = listeners.indexOf(setState);
      if (index > -1) {
        listeners.splice(index, 1);
      }
    };
  }, []);

  return {
    ...state,
    toast,
    dismiss: dismissToast,
    pause: pauseAutoDismiss,
    resume: resumeAutoDismiss,
  };
}

/** Test isolation: the module-level store otherwise outlives one render. */
function clearToasts(): void {
  for (const timer of toastTimers.values()) clearTimeout(timer);
  toastTimers.clear();
  toastDeadlines.clear();
  pausedToastRemaining.clear();
  memoryState = { toasts: [] };
  listeners.forEach((listener) => {
    listener(memoryState);
  });
}

export { useToast, toast, clearToasts };
