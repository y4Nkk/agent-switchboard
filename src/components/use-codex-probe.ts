import { uiMessage } from "../i18n/errors";
import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelCodexProbe, getCurrentCodexProbe, getCodexProbeCatalog, retryCodexProbeSave,
  startCodexProbe, type CodexProbeBatch, type CodexProbeCatalog, type CodexProbeRequest,
} from "../api/client";

const POLL_INTERVAL_MS = 1000;

function useProbeCatalog(enabled: boolean, epoch: number) {
  const [catalog, setCatalog] = useState<CodexProbeCatalog | null>(null);
  const [catalogError, setCatalogError] = useMessageState();
  useEffect(() => {
    if (!enabled || catalog !== null) return;
    let active = true;
    getCodexProbeCatalog().then((next) => {
      if (active) { setCatalog(next); setCatalogError(null); }
    }).catch((reason) => {
      if (active) setCatalogError(reason);
    });
    return () => { active = false; };
  }, [enabled, catalog, epoch, setCatalogError]);
  return { questions: catalog?.questions ?? null,
    customTimeoutSeconds: catalog?.customTimeoutSeconds ?? null,
    customMaxRuns: catalog?.customMaxRuns ?? null, catalogError };
}

/** Reads the live-or-latest batch from the backend. Polling continues only
 * while a batch is actually running, so a refresh or restart reconnects
 * without the frontend remembering any id. */
function useProbePolling(enabled: boolean, epoch: number) {
  const [read, setRead] = useState<{ epoch: number; status: CodexProbeBatch | null } | null>(null);
  const [readError, setReadError] = useMessageState();
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = async () => {
      let keepPolling = false;
      try {
        const next = await getCurrentCodexProbe();
        if (!active) return;
        setRead({ epoch, status: next });
        setReadError(null);
        // Results that could not be saved leave nothing to poll.
        keepPolling = next !== null && next.status === "running" && !next.persistPending;
      } catch (reason) {
        if (active) setReadError(reason);
        keepPolling = true;
      }
      if (active && keepPolling) timer = setTimeout(() => void tick(), POLL_INTERVAL_MS);
    };
    void tick();
    return () => { active = false; clearTimeout(timer); };
  }, [enabled, epoch, setReadError]);
  return { status: read?.status ?? null, synchronized: read?.epoch === epoch && readError === null,
    readError, setReadError };
}

function useProbeRun(enabled: boolean, bumpEpoch: () => void,
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
    if (!enabled || startPending.current) return;
    startPending.current = true;
    setStarting(true);
    setStartError(null);
    setCancelError(null);
    setRetryError(null);
    setReadError(null);
    try {
      await startCodexProbe(request);
      if (alive.current) bumpEpoch();
    } catch (reason) {
      if (alive.current) { setStartError(reason); bumpEpoch(); }
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
      if (alive.current) setCancelError(reason);
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
      if (alive.current) bumpEpoch();
    } catch (reason) {
      if (alive.current) setRetryError(reason);
    } finally {
      if (alive.current) setRetrying(false);
    }
  };

  return { starting, cancelling, retrying, startError, cancelError, retryError,
    start, cancel, retrySave };
}

export function useCodexProbe(enabled: boolean) {
  const [epoch, setEpoch] = useState(0);
  const catalog = useProbeCatalog(enabled, epoch);
  const bumpEpoch = useCallback(() => setEpoch((value) => value + 1), []);
  const { status, synchronized, readError, setReadError } = useProbePolling(enabled, epoch);
  const run = useProbeRun(enabled, bumpEpoch, setReadError);
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
