import { uiMessage } from "../i18n/errors";
import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelCodexProbe, getCurrentCodexProbe, getCodexProbeCatalog, retryCodexProbeSave,
  startCodexProbe, type CodexProbeBatch, type CodexProbeCatalog, type CodexProbeRequest,
} from "../api/client";
import { notifyProbeError, useProbeEvents, useProbeNotifications } from "./useProbeNotifications";

const POLL_INTERVAL_MS = 1000;

function useProbeCatalog(epoch: number) {
  const [catalog, setCatalog] = useState<CodexProbeCatalog | null>(null);
  const [catalogError, setCatalogError] = useMessageState();
  useEffect(() => {
    if (catalog !== null) return;
    let active = true;
    getCodexProbeCatalog().then((next) => {
      if (active) { setCatalog(next); setCatalogError(null); }
    }).catch((reason) => {
      if (active) setCatalogError(reason);
    });
    return () => { active = false; };
  }, [catalog, epoch, setCatalogError]);
  return { questions: catalog?.questions ?? null,
    customTimeoutSeconds: catalog?.customTimeoutSeconds ?? null,
    customMaxRuns: catalog?.customMaxRuns ?? null, catalogError };
}

/** One application-wide reader, independent of workspace/window visibility. */
function useProbePolling(epoch: number) {
  const [read, setRead] = useState<{ epoch: number; status: CodexProbeBatch | null } | null>(null);
  const [readError, setReadError] = useMessageState();
  const dismissReadError = useRef<(() => void) | null>(null);
  useEffect(() => () => { dismissReadError.current?.(); }, []);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = async () => {
      let keepPolling = false;
      try {
        const next = await getCurrentCodexProbe();
        if (!active) return;
        setRead({ epoch, status: next });
        setReadError(null);
        dismissReadError.current?.();
        dismissReadError.current = null;
        // Results that could not be saved leave nothing to poll.
        keepPolling = next !== null && next.status === "running" && !next.persistPending;
      } catch (reason) {
        if (active) {
          setReadError(reason);
          dismissReadError.current ??= notifyProbeError("read", reason);
        }
        keepPolling = true;
      }
      if (active && keepPolling) timer = setTimeout(() => void tick(), POLL_INTERVAL_MS);
    };
    void tick();
    return () => { active = false; clearTimeout(timer); };
  }, [epoch, setReadError]);
  return { status: read?.status ?? null, synchronized: read?.epoch === epoch && readError === null,
    readError, setReadError };
}

function useProbeRun(bumpEpoch: () => void, watch: (batchId: string) => void,
  setReadError: (error: string | null) => void) {
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [startError, setStartError] = useMessageState();
  const [cancelError, setCancelError] = useMessageState();
  const [retryError, setRetryError] = useMessageState();
  const startPending = useRef(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  const start = async (request: CodexProbeRequest) => {
    if (startPending.current) return;
    startPending.current = true;
    setStarting(true);
    setStartError(null);
    setCancelError(null);
    setRetryError(null);
    setReadError(null);
    try {
      const batchId = await startCodexProbe(request);
      if (alive.current) { watch(batchId); bumpEpoch(); }
    } catch (reason) {
      if (alive.current) { setStartError(reason); notifyProbeError("start", reason); bumpEpoch(); }
    } finally {
      startPending.current = false;
      if (alive.current) setStarting(false);
    }
  };

  const cancel = async () => {
    if (cancelling) return;
    setCancelling(true);
    setCancelError(null);
    try {
      const accepted = await cancelCodexProbe();
      if (alive.current && !accepted) setCancelError(uiMessage("codex.probe.cancelNoop"));
    } catch (reason) {
      if (alive.current) { setCancelError(reason); notifyProbeError("cancel", reason); }
    } finally {
      if (alive.current) { setCancelling(false); bumpEpoch(); }
    }
  };

  const retrySave = async () => {
    if (retrying) return;
    setRetrying(true);
    setRetryError(null);
    try {
      await retryCodexProbeSave();
    } catch (reason) {
      if (alive.current) { setRetryError(reason); notifyProbeError("save", reason); }
    } finally {
      if (alive.current) { setRetrying(false); bumpEpoch(); }
    }
  };

  return { starting, cancelling, retrying, startError, cancelError, retryError,
    start, cancel, retrySave };
}

export function useCodexProbe() {
  const [epoch, setEpoch] = useState(0);
  const catalog = useProbeCatalog(epoch);
  const bumpEpoch = useCallback(() => setEpoch((value) => value + 1), []);
  const { status, synchronized, readError, setReadError } = useProbePolling(epoch);
  const watch = useProbeNotifications(status, synchronized);
  useProbeEvents(bumpEpoch, watch);
  const run = useProbeRun(bumpEpoch, watch, setReadError);
  const progress = status?.status === "running" ? status.progress : null;
  const stopping = progress?.kind === "preparing" ? progress.cancelling : progress?.phase === "cancelling";
  return {
    ...catalog, status, readError, synchronized,
    ...run, cancelling: run.cancelling || stopping,
    running: status?.status === "running",
    /** One-shot re-read, e.g. after history deletions. */
    refresh: bumpEpoch,
  };
}
