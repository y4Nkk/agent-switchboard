import { useEffect } from "react";
import { CloseIcon } from "./icons";
import { ToastStatusIcon } from "./ToastStatusIcon";
import { useToast, type Toast } from "./use-toast";

/**
 * Global floating notifications (spiralcoder structure replica, user
 * directive 2026-08-31): same stacking limit, error-first ordering, kind
 * based auto-dismiss and hover/focus/hidden pause contract; visuals use this
 * system's tokens (DESIGN.md §8 全局通知).
 */
export function Toaster() {
  const { toasts, dismiss, pause, resume } = useToast();

  useEffect(() => {
    const handleVisibilityChange = () => {
      for (const toast of toasts) {
        if (document.hidden) pause(toast.id);
        else resume(toast.id);
      }
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => document.removeEventListener("visibilitychange", handleVisibilityChange);
  }, [pause, resume, toasts]);

  if (toasts.length === 0) return null;

  return (
    <div className="asb-toast-stack">
      {toasts.map((toast) => (
        <ToastItem
          key={toast.id}
          toast={toast}
          onDismiss={dismiss}
          onPause={pause}
          onResume={resume}
        />
      ))}
    </div>
  );
}

interface ToastItemProps {
  toast: Toast;
  onDismiss: (toastId: string) => void;
  onPause: (toastId: string) => void;
  onResume: (toastId: string) => void;
}

function ToastItem({ toast, onDismiss, onPause, onResume }: ToastItemProps) {
  const { id, title, description, kind } = toast;
  return (
    <div
      className="asb-toast"
      onPointerEnter={() => onPause(id)}
      onPointerLeave={() => onResume(id)}
      onFocus={() => onPause(id)}
      onBlur={() => onResume(id)}
    >
      <ToastStatusIcon kind={kind} />
      <div role={kind === "error" ? "alert" : "status"} aria-atomic="true" className="asb-toast-body">
        {title ? <div className="asb-toast-title">{title}</div> : null}
        {description ? <div className="asb-toast-description">{description}</div> : null}
      </div>
      <button
        type="button"
        className="asb-toast-close"
        aria-label="关闭通知"
        onClick={() => onDismiss(id)}
      >
        <CloseIcon />
      </button>
    </div>
  );
}
