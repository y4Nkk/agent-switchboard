import { Link2, Plug, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import type { GatewayStatus, GatewayStatusKind } from "../api/client";
import { useI18n, type MessageKey } from "../i18n";
import { Button } from "./Button";
import { CopyGatewayAddressButton } from "./GatewayPagePrimitives";
import { FactList } from "./FactList";

const STATUS_KEYS: Record<GatewayStatusKind, MessageKey> = {
  standby: "gateway.status.standby",
  running: "gateway.status.running",
  portConflict: "gateway.status.portConflict",
  bindRejected: "gateway.status.bindRejected",
  needsRepair: "gateway.status.needsRepair",
  recoveryBlocked: "gateway.status.recoveryBlocked",
};

interface Props {
  status: GatewayStatus;
  failed: boolean;
  refreshedAt: number | null;
  retrying: boolean;
  onRetry: () => void;
  onChangePort: () => void;
  active: boolean;
}

/** Runtime state and listener controls below the overview artwork. */
export function GatewayHub({ status, failed, refreshedAt, retrying, onRetry, onChangePort, active }: Props) {
  const { t } = useI18n();
  return (
    <div className={`asb-gateway-hub${failed ? " is-failure" : ""}`}>
      <div className="asb-gateway-hub-identity">
        <p className="asb-gateway-hub-state" role="status">
          <span className={`asb-gateway-lamp${failed ? " is-failure" : status.status === "running" ? " is-running" : ""}`} aria-hidden="true" />
          {t(STATUS_KEYS[status.status])}
        </p>
        <GatewayUpdatedAgo at={refreshedAt} active={active} />
      </div>
      {status.failure && (
        <div className="asb-gateway-hub-failure" role="alert">
          {status.status === "portConflict" && <p className="asb-gateway-hub-message">{t("gateway.portConflictHelp")}</p>}
          {status.status === "bindRejected" && <p className="asb-gateway-hub-message">{t("gateway.bindRejectedHelp")}</p>}
          <p className="asb-gateway-hub-detail">{t("gateway.failureDetail", { reason: status.failure.message })}</p>
        </div>
      )}
      <FactList inline rows={[
        { label: t("gateway.loopbackKicker"), icon: Link2, value: <div className="asb-gateway-fact-value is-address">
          {status.baseUrl ? <>
            <code>{status.baseUrl}</code>
            <CopyGatewayAddressButton value={status.baseUrl} />
          </> : <span className="asb-field-help">{t("gateway.addressNotListening")}</span>}
        </div> },
        { label: t("gateway.configuredPort"), icon: Wrench, value: <div className="asb-gateway-fact-value is-port">
          <span className="asb-gateway-port-value asb-num">{status.configuredPort}</span>
          <Button variant="secondary" size="compact" disabled={retrying || status.status === "recoveryBlocked"}
            onClick={onChangePort}>{t("gateway.changePort")}</Button>
        </div> },
        { label: t("gateway.actualPort"), icon: Plug, tone: status.failure ? "warning" : undefined,
          value: <div className="asb-gateway-fact-value is-port">
          {status.listeningPort === null
            ? <span className="asb-field-help">{t("gateway.portNotListening")}</span>
            : <span className="asb-gateway-port-value asb-num">{status.listeningPort}</span>}
          {status.failure && (
            <Button variant="secondary" size="compact" disabled={retrying} onClick={onRetry}>{t("gateway.retryListen")}</Button>
          )}
        </div> },
      ]} />
    </div>
  );
}

/** Only the timestamp rerenders each second; the topology follows snapshots. */
function GatewayUpdatedAgo({ at, active }: { at: number | null; active: boolean }) {
  const { t } = useI18n();
  const [, setTick] = useState(0);
  useEffect(() => {
    if (at === null || !active) return;
    const timer = window.setInterval(() => setTick((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [at, active]);
  if (at === null) return null;
  const seconds = Math.max(0, Math.round((Date.now() - at) / 1000));
  const text = seconds < 5
    ? t("gateway.updatedJustNow")
    : seconds < 60
      ? t("gateway.updatedSecondsAgo", { seconds })
      : t("gateway.updatedMinutesAgo", { minutes: Math.floor(seconds / 60) });
  return <p className="asb-gateway-hub-updated">{text}</p>;
}
