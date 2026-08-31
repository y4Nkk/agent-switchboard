import { useCallback, useState } from "react";
import { checkUpdate, type CommandError, type UpdateCheck } from "../api/client";

interface UpdateCheckDeps {
  busy: boolean;
  onError: (error: CommandError) => void;
  clearError: () => void;
  setBusy: (busy: boolean) => void;
}

/** Manual software-update check against the project's GitHub releases.
 * Informational only: the result feeds the settings-page section and the
 * title-bar indicator; nothing is downloaded or installed. */
export function useUpdateCheck({ busy, onError, clearError, setBusy }: UpdateCheckDeps) {
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);

  const runUpdateCheck = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    clearError();
    try {
      setUpdateCheck(await checkUpdate());
    } catch (caught) {
      onError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, clearError, onError, setBusy]);

  return { updateCheck, runUpdateCheck };
}
