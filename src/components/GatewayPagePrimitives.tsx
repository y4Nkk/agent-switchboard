import { useState } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { CopyButton } from "./CopyButton";

export function ConfirmGatewayRecoveryDiscard({ onConfirm, busy }: { onConfirm: () => Promise<void>; busy: boolean }) {
  const { t } = useI18n();
  const [confirming, setConfirming] = useState(false);
  if (!confirming) {
    return (
      <div>
        <Button variant="secondary" disabled={busy} onClick={() => setConfirming(true)}>
          {t("gateway.discardRecordButton")}
        </Button>
      </div>
    );
  }
  return (
    <ConfirmSheet title={t("gateway.discardRecordButton")} busy={busy} destructive
      confirmLabel={t("gateway.discardConfirmButton")} onCancel={() => setConfirming(false)}
      onConfirm={() => void onConfirm().finally(() => setConfirming(false))}>
      <span className="asb-gateway-guidance-copy">
        {t("gateway.discardConfirmCopy")}
      </span>
    </ConfirmSheet>
  );
}

export function CopyGatewayAddressButton({ value }: { value: string }) {
  const { t } = useI18n();
  return (
    <CopyButton content={value} iconOnly
      label={t("gateway.copyAddressAria")}
      copiedLabel={t("gateway.copiedAddressAria")} />
  );
}
