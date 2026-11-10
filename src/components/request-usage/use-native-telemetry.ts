import { useEffect, useState } from "react";
import { commitNativeTelemetry, previewNativeTelemetry, readNativeTelemetry, retryNativeTelemetry,
  type AppKind, type ClientConfigurationApplyPreview, type NativeTelemetrySetup } from "../../api/client";
import { useMessageState } from "../../i18n/use-message-state";

export interface PendingTelemetryExport { app: AppKind; enabled: boolean; preview: ClientConfigurationApplyPreview }

export function useNativeTelemetry(active: boolean, onApplied: () => void) {
  const [data, setData] = useState<NativeTelemetrySetup | null>(null);
  const [pending, setPending] = useState<PendingTelemetryExport | null>(null);
  const [busy, setBusy] = useState(false);
  const [applied, setApplied] = useState(false);
  const [error, setError] = useMessageState();
  const [readError, setReadError] = useMessageState();
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (!active) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const read = async () => {
      try {
        const snapshot = await readNativeTelemetry();
        if (!stopped) { setData(snapshot); setReadError(null); }
      } catch (error) { if (!stopped) setReadError(error); }
      finally { if (!stopped) timer = setTimeout(() => void read(), 30_000); }
    };
    void read();
    return () => { stopped = true; if (timer) clearTimeout(timer); };
  }, [active, revision, setReadError]);

  const preview = async (app: AppKind, enabled: boolean) => {
    setBusy(true); setError(null); setApplied(false);
    try { setPending({ app, enabled, preview: await previewNativeTelemetry(app, enabled) }); }
    catch (error) { setError(error); }
    finally { setBusy(false); }
  };
  const commit = async () => {
    if (!pending) return;
    setBusy(true); setError(null);
    try {
      await commitNativeTelemetry(pending.app, pending.enabled, pending.preview);
      setPending(null); setApplied(true); setRevision((value) => value + 1); onApplied();
    } catch (error) { setError(error); }
    finally { setBusy(false); }
  };
  const retry = async () => {
    setBusy(true); setError(null);
    try { await retryNativeTelemetry(); }
    catch (error) { setError(error); }
    finally { setBusy(false); setRevision((value) => value + 1); }
  };
  return { data, pending, busy, applied, error, readError, preview, commit, retry,
    cancel: () => { setPending(null); setError(null); } };
}
