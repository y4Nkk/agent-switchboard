import { uiMessage } from "../i18n/errors";
import { useMessageState } from "../i18n/use-message-state";
import { useEffect, useRef, useState } from "react";
import {
  checkCodexResetHubStatus,
  checkCodexResetStatus,
  getCachedCodexOfficialReset,
  getCachedCodexResetHubStatus,
  getCachedCodexResetStatus,
  refreshCodexOfficialReset,
  type CodexOfficialQuota,
  type CodexOfficialQuotaStatus,
  type CodexQuotaReadFailure,
  type CodexResetRead,
} from "../api/client";
import { officialQuotaFailureMessage } from "../lib/usage-format";
import { useUsageHistory } from "./use-usage-history";

function statusCopy(status: Exclude<CodexOfficialQuotaStatus, "available">, failure: CodexQuotaReadFailure | null) {
  switch (status) {
    case "signInRequired":
      return uiMessage("clientConfig.quota.statusSignIn");
    case "reauthenticationRequired":
      return uiMessage("clientConfig.quota.statusReauth");
    case "unavailable":
      return officialQuotaFailureMessage(failure);
  }
}

/** The machine's own Codex official quota read: cache first, explicit refresh
 * from the page header. Only an available read is displayable; every other
 * shape (including a null cache) renders the empty state. */
export function useCodexOfficialReset(enabled: boolean) {
  const history = useUsageHistory({ kind: "official" }, enabled);
  const [snapshot, setSnapshot] = useState<CodexOfficialQuota | null>(null);
  const [freshness, setFreshness] = useState<"cached" | "live">("cached");
  const [cacheLoading, setCacheLoading] = useState(true);
  const [cacheError, setCacheError] = useMessageState();
  const [loading, setLoading] = useState(false);
  const [readError, setReadError] = useMessageState();
  const [statusNotice, setStatusNotice] = useMessageState();
  const requestRef = useRef<Promise<CodexOfficialQuota> | null>(null);
  const cacheRevisionRef = useRef(0);

  useEffect(() => {
    if (!enabled) return undefined;
    let active = true;
    const revision = ++cacheRevisionRef.current;

    const loadCache = async () => {
      try {
        const cached = await getCachedCodexOfficialReset();
        if (active && cacheRevisionRef.current === revision) setSnapshot(cached);
      } catch (reason) {
        if (active && cacheRevisionRef.current === revision) setCacheError(reason);
      } finally {
        if (active && cacheRevisionRef.current === revision) setCacheLoading(false);
      }
    };

    void loadCache();
    return () => {
      active = false;
    };
  }, [enabled]);

  const readStatus = async () => {
    if (!enabled || requestRef.current !== null) return;

    cacheRevisionRef.current += 1;
    setCacheLoading(false);
    setCacheError(null);
    setLoading(true);
    setReadError(null);
    setStatusNotice(null);
    const request = Promise.resolve().then(() => refreshCodexOfficialReset());
    requestRef.current = request;

    try {
      const next = await request;
      if (next.status === "available") {
        setSnapshot(next);
        setFreshness("live");
        void history.refresh();
      } else {
        setStatusNotice(statusCopy(next.status, next.failure));
      }
    } catch (reason) {
      setReadError(reason);
    } finally {
      if (requestRef.current === request) requestRef.current = null;
      setLoading(false);
    }
  };

  const quota = snapshot !== null && snapshot.status === "available" ? snapshot : null;

  return {
    quota,
    freshness,
    cacheLoading,
    cacheError,
    loading,
    readError,
    statusNotice,
    readStatus,
    history,
  };
}

/** One source's read state: cache first, explicit refresh from the page
 * header. Both public reset monitors share this exact shape. */
interface ResetSourceRead<S> {
  snapshot: CodexResetRead<S> | null;
  cacheLoading: boolean;
  cacheError: string | null;
  loading: boolean;
  readError: string | null;
  readStatus: () => Promise<void>;
}

function useResetSourceRead<S>(
  enabled: boolean,
  loadCache: () => Promise<CodexResetRead<S> | null>,
  check: () => Promise<CodexResetRead<S>>,
): ResetSourceRead<S> {
  const [snapshot, setSnapshot] = useState<CodexResetRead<S> | null>(null);
  const [cacheLoading, setCacheLoading] = useState(true);
  const [cacheError, setCacheError] = useMessageState();
  const [loading, setLoading] = useState(false);
  const [readError, setReadError] = useMessageState();
  const requestRef = useRef<Promise<CodexResetRead<S>> | null>(null);
  const cacheRevisionRef = useRef(0);

  useEffect(() => {
    if (!enabled) return undefined;
    let active = true;
    const revision = ++cacheRevisionRef.current;

    const load = async () => {
      try {
        const cached = await loadCache();
        if (active && cacheRevisionRef.current === revision) setSnapshot(cached);
      } catch (reason) {
        if (active && cacheRevisionRef.current === revision) setCacheError(reason);
      } finally {
        if (active && cacheRevisionRef.current === revision) setCacheLoading(false);
      }
    };

    void load();
    return () => {
      active = false;
    };
    // The loaders are stable module functions, not props.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  const readStatus = async () => {
    if (!enabled || requestRef.current !== null) return;

    cacheRevisionRef.current += 1;
    setCacheLoading(false);
    setCacheError(null);
    setLoading(true);
    setReadError(null);
    const request = Promise.resolve().then(check);
    requestRef.current = request;

    try {
      setSnapshot(await request);
    } catch (reason) {
      setReadError(reason);
    } finally {
      if (requestRef.current === request) requestRef.current = null;
      setLoading(false);
    }
  };

  return { snapshot, cacheLoading, cacheError, loading, readError, readStatus };
}

export type CodexResetSource = "runway" | "hub";

/** The two explicit, read-only public reset monitors: cache first, one
 * refresh action in the page header reading both; every error and freshness
 * state is reported per source. */
export function useCodexResetSignal(enabled: boolean) {
  const runway = useResetSourceRead(enabled, getCachedCodexResetStatus, checkCodexResetStatus);
  const hub = useResetSourceRead(enabled, getCachedCodexResetHubStatus, checkCodexResetHubStatus);
  return {
    runway,
    hub,
    loading: runway.loading || hub.loading,
    readStatus: () => {
      void runway.readStatus();
      void hub.readStatus();
    },
  };
}
