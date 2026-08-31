import { useCallback, useState } from "react";
import type { CommandError } from "../api/client";
import { toast } from "../components/use-toast";

/**
 * The cross-domain operation frame: one busy flag and the error gate.
 * Persistent store-reset errors keep the actionable banner; every other
 * failure becomes a transient global toast (longer 10s error dwell).
 */
export function useOperationFrame() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);

  const reportError = useCallback((caught: CommandError) => {
    if (caught.code === "profile-store-unsupported") {
      setError(caught);
      return;
    }
    toast({ kind: "error", title: "操作失败", description: caught.message });
  }, []);

  const clearError = useCallback(() => setError(null), []);

  return { busy, setBusy, error, reportError, clearError };
}
