import type { NativeTelemetrySetup as Setup, AppKind } from "../../api/client";
import { useId, useState } from "react";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { Collapsible, CollapsibleTrigger } from "../Collapsible";
import { ConfirmSheet } from "../ConfirmSheet";
import { DiffView } from "../DiffView";
import { Time } from "../Time";
import { useNativeTelemetry, type PendingTelemetryExport } from "./use-native-telemetry";

export function NativeTelemetrySetup({ active, onApplied }: { active: boolean; onApplied: () => void }) {
  const { t } = useI18n();
  const state = useNativeTelemetry(active, onApplied);
  const id = useId();
  const [open, setOpen] = useState(false);
  const receiver = state.data?.receiver;
  const listening = receiver?.listening && !state.readError;
  return <div className="asb-native-telemetry-setup">
    <div className="asb-native-telemetry-toolbar">
      <p className="asb-request-usage-status" role="status">
        <span className={`asb-status-pill ${listening ? "is-ok" : "is-idle"}`}>
          <span className="asb-status-pill-dot" aria-hidden="true" />
          {t("usagePerformance.receiver")} · {t(state.readError ? "usagePerformance.receiverUnavailable" : receiver
            ? receiver.listening ? "usagePerformance.listening" : "usagePerformance.notListening" : "usagePerformance.loading")}
        </span>
        {receiver?.lastExportAtMs != null && <span>{t("usagePerformance.lastExport")}
          <Time iso={new Date(receiver.lastExportAtMs).toISOString()} mode="relative" /></span>}
      </p>
      <div className="asb-panel-actions">
        {receiver && !receiver.listening && <Button size="compact" variant="secondary" disabled={state.busy} onClick={() => void state.retry()}>
          {t("usagePerformance.retryListener")}</Button>}
        <CollapsibleTrigger open={open} onOpenChange={setOpen} contentId={id} label={t("usagePerformance.exportSettings")} />
      </div>
    </div>
    {state.readError && <p className="asb-field-error" role="alert">{state.readError}</p>}
    {receiver?.error && <p className="asb-field-error" role="alert">{receiver.error}</p>}
    {state.error && !state.pending && <p className="asb-field-error" role="alert">{state.error}</p>}
    {state.data?.clients.map((client) => client.error && <p key={client.app} className="asb-field-error" role="alert">
      {clientName(client.app)} · {client.error}
    </p>)}
    {state.applied && <p className="asb-scope-note" role="status">{t("usagePerformance.exportApplied")}</p>}
    <Collapsible open={open} id={id}>
      <div className="asb-native-telemetry-content">
        {state.data && <ExportClients clients={state.data.clients} busy={state.busy}
          listening={Boolean(listening)} onPreview={(app, enabled) => void state.preview(app, enabled)} />}
        {receiver && <p className="asb-request-usage-status">
          <code>{receiver.endpoint}</code>
          <span>{t("usagePerformance.acceptedRecords")} {receiver.acceptedRecords}</span>
          <span>{t("usagePerformance.ignoredRecords")} {receiver.ignoredRecords}</span>
        </p>}
      </div>
    </Collapsible>
    {state.pending && <ExportPreview pending={state.pending} busy={state.busy} error={state.error}
      onConfirm={() => void state.commit()} onCancel={state.cancel} />}
  </div>;
}

function ExportClients({ clients, busy, listening, onPreview }: {
  clients: Setup["clients"]; busy: boolean; listening: boolean;
  onPreview: (app: AppKind, enabled: boolean) => void;
}) {
  const { t } = useI18n();
  return <div className="asb-native-telemetry-clients">
    {clients.map((client) => <div key={client.app} className="asb-native-telemetry-client">
      <div><strong>{clientName(client.app)}</strong><span className="asb-request-usage-caption">
        {t(client.error ? "usagePerformance.exportUnavailable" : client.configured
          ? "usagePerformance.exportConfigured" : "usagePerformance.exportNotConfigured")}
      </span></div>
      <Button size="compact" variant="secondary" disabled={busy || Boolean(client.error) || (!listening && !client.configured)}
        onClick={() => onPreview(client.app, !client.configured)}>
        {t(client.configured ? "usagePerformance.disableExport" : "usagePerformance.enableExport")}
      </Button>
    </div>)}
  </div>;
}

function ExportPreview({ pending, busy, error, onConfirm, onCancel }: {
  pending: PendingTelemetryExport; busy: boolean; error: string | null; onConfirm: () => void; onCancel: () => void;
}) {
  const { t } = useI18n();
  return <ConfirmSheet title={t("usagePerformance.exportPreview", { client: clientName(pending.app) })}
    busy={busy} confirmLabel={t("usagePerformance.applyExport")} onConfirm={onConfirm} onCancel={onCancel}>
    <p>{t("usagePerformance.exportScope")}</p>
    <p><code>{pending.preview.file.preview.target}</code></p>
    {pending.preview.file.preview.warnings.map((warning) => <p className="asb-field-error" key={warning.key}>
      {localizedMessageText(warning, t)}</p>)}
    <DiffView changes={pending.preview.file.preview.changes} label={t("usagePerformance.exportChanges")} />
    {error && <p className="asb-field-error" role="alert">{error}</p>}
  </ConfirmSheet>;
}
