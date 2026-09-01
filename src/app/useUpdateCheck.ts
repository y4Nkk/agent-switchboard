import { useCallback, useEffect, useRef, useState } from "react";
import { checkUpdate, type CommandError, type UpdateCheck } from "../api/client";

interface UpdateCheckDeps {
  onError: (error: CommandError) => void;
}

/** One startup check plus user-triggered checks against the project's GitHub
 * releases. The check never blocks the application shell, downloads, or
 * installs; a new release only enables the explicit update entry. */
export function useUpdateCheck({ onError }: UpdateCheckDeps) {
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const inFlight = useRef(false);
  const automaticCheckStarted = useRef(false);

  const runUpdateCheck = useCallback(async (reportFailure: boolean) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setChecking(true);
    try {
      setUpdateCheck(await checkUpdate());
    } catch (caught) {
      if (reportFailure) onError(caught as CommandError);
    } finally {
      inFlight.current = false;
      setChecking(false);
    }
  }, [onError]);

  useEffect(() => {
    if (automaticCheckStarted.current) return;
    automaticCheckStarted.current = true;
    void runUpdateCheck(false);
  }, [runUpdateCheck]);

  return {
    updateCheck,
    checking,
    runUpdateCheck: () => runUpdateCheck(true),
  };
}
