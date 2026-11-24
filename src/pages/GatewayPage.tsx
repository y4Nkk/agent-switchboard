import { useMessageState } from "../i18n/use-message-state";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button } from "../components/Button";
import type { ClientFilterValue } from "../components/ClientFilter";
import { GatewayGuidance, GatewayTopology } from "../components/GatewayTopology";
import { GatewayHub } from "../components/GatewayHub";
import { GatewayPortChangeSheet, type PortSheetState } from "../components/GatewayPortChangeSheet";
import { GatewayTelemetry } from "../components/GatewayTelemetry";
import { useI18n } from "../i18n";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import {
  discardGatewayPortChange,
  getGatewayStatus,
  retryGatewayBind,
  type GatewayStatus,
  type GatewayStatusKind,
  type ProviderProfile,
} from "../api/client";

const POLL_INTERVAL_MS = 5000;
const FAILURE_STATES: ReadonlySet<GatewayStatusKind> = new Set([
  "portConflict",
  "bindRejected",
  "needsRepair",
  "recoveryBlocked",
]);

interface Props {
  busy: boolean;
  /** False while another page is shown; polling runs only when visible. */
  active: boolean;
  profiles: readonly Pick<ProviderProfile, "id" | "name">[];
}

/** Runtime snapshots and transient focus share one page owner. */
export function GatewayPage({ active, busy, profiles, routing }: Props & {
  routing: (runtime: { summary: ReactNode; observation: ReactNode; mutating: boolean; visible: boolean }) => ReactNode;
}) {
  const [focus, setFocus] = useState<ClientFilterValue>("all");
  const [portSheet, setPortSheet] = useState<PortSheetState | null>(null);
  const visible = useWorkspaceVisible(active);
  // The sheet is portaled over the whole window; leaving the gateway workspace
  // must close it so it never floats over an unrelated page. The flow hook
  // releases any held preparation on unmount.
  useEffect(() => {
    if (!active) setPortSheet(null);
  }, [active]);
  const { status, error, retrying, refreshedAt, refresh, retryBind, discardRecovery } = useGatewayRuntime(visible);
  const profileNames = useMemo(() => {
    const names = new Map<string, string>();
    for (const profile of profiles) names.set(profile.id, profile.name);
    return names;
  }, [profiles]);

  const failed = status ? FAILURE_STATES.has(status.status) : false;
  const summary = status ? <>
    {error && <p className="asb-fail-text" role="alert">{error}</p>}
    <GatewayHub status={status} failed={failed} refreshedAt={refreshedAt} retrying={retrying || busy} active={visible}
      onRetry={retryBind} onChangePort={() => setPortSheet({ stage: "input" })} />
    <GatewayGuidance status={status} busy={retrying || busy} onDiscardRecovery={discardRecovery} />
  </> : error ? <GatewayUnavailable error={error} onRetry={refresh} /> : <GatewayLoading />;
  const observation = visible && status ? (
    <div className="asb-gateway-observation">
      <GatewayTopology status={status} failed={failed} profileNames={profileNames}
        focus={focus} onFocusChange={setFocus} />
      <GatewayTelemetry metrics={status.metrics} profileNames={profileNames} focus={focus} />
    </div>
  ) : null;
  return (
    <>
      {routing({ summary, observation, mutating: retrying, visible })}
      {portSheet && status && (
        <GatewayPortChangeSheet
          state={portSheet}
          configuredPort={status.configuredPort}
          onState={setPortSheet}
          onRefreshed={() => void refresh()}
        />
      )}
    </>
  );
}

function useGatewayRuntime(active: boolean) {
  const [status, setStatus] = useState<GatewayStatus | null>(null);
  const [refreshedAt, setRefreshedAt] = useState<number | null>(null);
  const [error, setError] = useMessageState();
  const [retrying, setRetrying] = useState(false);
  const mutation = useRef(false);
  const generation = useRef(0);
  const visible = useRef(active);
  visible.current = active;
  const flight = useRef<Promise<void> | null>(null);
  const refresh = useCallback(async () => {
    const previous = generation.current;
    if (flight.current) await flight.current;
    if (!visible.current || mutation.current || previous !== generation.current) return;
    const request = ++generation.current;
    const pending = (async () => {
      try {
        const snapshot = await getGatewayStatus();
        if (!visible.current || request !== generation.current) return;
        setStatus(snapshot); setRefreshedAt(Date.now()); setError(null);
      } catch (cause) {
        if (visible.current && request === generation.current) setError(cause);
      }
    })();
    flight.current = pending;
    await pending;
    if (flight.current === pending) flight.current = null;
  }, []);

  useEffect(() => {
    if (!active) return;
    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      await refresh();
      if (current) timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
    };
    void poll();
    return () => { current = false; generation.current += 1; clearTimeout(timer); };
  }, [active, refresh]);

  const mutate = useCallback(async (action: () => Promise<GatewayStatus>) => {
    if (mutation.current) return;
    mutation.current = true;
    generation.current += 1;
    setRetrying(true);
    try {
      setStatus(await action());
      setRefreshedAt(Date.now());
      setError(null);
    } catch (cause) {
      setError(cause);
    } finally {
      mutation.current = false;
      setRetrying(false);
    }
  }, []);

  return { status, error, retrying, refreshedAt, refresh,
    retryBind: () => mutate(retryGatewayBind), discardRecovery: () => mutate(() => discardGatewayPortChange(true)) };
}

function GatewayUnavailable({ error, onRetry }: { error: string; onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <div className="asb-gateway-summary-state">
      <div role="alert">
        <p className="asb-gateway-hub-state">{t("gateway.unavailableTitle")}</p>
        <p className="asb-gateway-hub-detail">{error}</p>
      </div>
      <Button variant="secondary" onClick={() => void onRetry()}>{t("gateway.retry")}</Button>
    </div>
  );
}

function GatewayLoading() {
  const { t } = useI18n();
  return (
    <div className="asb-gateway-summary-state" role="status">
      <p className="asb-gateway-hub-state">{t("gateway.loadingAriaLabel")}</p>
      <div className="asb-gateway-loading" aria-hidden="true">
        <span className="asb-skeleton" />
        <span className="asb-skeleton" />
        <span className="asb-skeleton" />
      </div>
    </div>
  );
}
