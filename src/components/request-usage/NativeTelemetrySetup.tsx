import { Database, ListFilter, Network } from "lucide-react";
import type { NativeTelemetrySetup as Setup, AppKind } from "../../api/client";
import { useState } from "react";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { clientName } from "../../lib/client-name";
import { AppDialog, AppDialogSuspension } from "../AppDialog";
import { Button } from "../Button";
import { ConfirmSheet } from "../ConfirmSheet";
import { DiffView } from "../DiffView";
import { FactList } from "../FactList";
import { Switch } from "../Switch";
import { Time } from "../Time";
import { useNativeTelemetry, type PendingTelemetryExport } from "./use-native-telemetry";

export function NativeTelemetrySetup({ active, onApplied }: { active: boolean; onApplied: () => void }) {
  const { t } = useI18n();
  const state = useNativeTelemetry(active, onApplied);
  const [open, setOpen] = useState(false);
  const receiver = state.data?.receiver;
  const listening = receiver?.listening && !state.readError;
  return <section className="asb-native-telemetry-setup asb-usage-surface" aria-label={t("usagePerformance.receiver")}>
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
        <Button size="compact" variant="secondary" aria-haspopup="dialog" onClick={() => setOpen(true)}>
          {t("usagePerformance.exportSettings")}
        </Button>
      </div>
    </div>
    {open ? <CollectionSettingsDialog state={state} onClose={() => setOpen(false)} /> : <CollectionFeedback state={state} />}
  </section>;
}

type CollectionState = ReturnType<typeof useNativeTelemetry>;

function CollectionFeedback({ state }: { state: CollectionState }) {
  const { t } = useI18n();
  return <>
    {state.readError && <p className="asb-field-error" role="alert">{state.readError}</p>}
    {state.data?.receiver.error && <p className="asb-field-error" role="alert">{state.data.receiver.error}</p>}
    {state.error && !state.pending && <p className="asb-field-error" role="alert">{state.error}</p>}
    {state.applied && <p className="asb-scope-note" role="status">{t("usagePerformance.exportApplied")}</p>}
  </>;
}

function CollectionSettingsDialog({ state, onClose }: { state: CollectionState; onClose: () => void }) {
  const { t, language } = useI18n();
  const { data, readError, pending, busy } = state;
  const receiver = data?.receiver;
  const number = new Intl.NumberFormat(language);
  return <>
    <AppDialogSuspension suspended={pending !== null}>
      <AppDialog title={t("usagePerformance.exportSettings")} busy={busy} onClose={onClose}>
        <div className="asb-native-telemetry-content">
          <CollectionFeedback state={state} />
          {!data && !readError && <p className="asb-request-usage-status" role="status">{t("usagePerformance.loading")}</p>}
          {data && <ExportClients clients={data.clients} busy={busy}
            listening={data.receiver.listening && !readError} onPreview={(app, enabled) => void state.preview(app, enabled)} />}
          {receiver && <div className="asb-native-telemetry-receiver">
            <div className="asb-native-telemetry-toolbar">
              <h3 className="asb-group-title">{t("usagePerformance.receiver")}</h3>
              {!receiver.listening && <Button size="compact" variant="secondary" disabled={busy} onClick={() => void state.retry()}>
                {t("usagePerformance.retryListener")}
              </Button>}
            </div>
            <FactList rows={[
              { label: t("usagePerformance.receiverEndpoint"), icon: Network, value: receiver.endpoint, format: "code" },
              { label: t("usagePerformance.acceptedRecords"), icon: Database, value: number.format(receiver.acceptedRecords), format: "number" },
              { label: t("usagePerformance.ignoredRecords"), icon: ListFilter, value: number.format(receiver.ignoredRecords), format: "number" },
            ]} />
          </div>}
        </div>
      </AppDialog>
    </AppDialogSuspension>
    {state.pending && <ExportPreview pending={state.pending} busy={state.busy} error={state.error}
      onConfirm={() => void state.commit()} onCancel={state.cancel} />}
  </>;
}

function ExportClients({ clients, busy, listening, onPreview }: {
  clients: Setup["clients"]; busy: boolean; listening: boolean;
  onPreview: (app: AppKind, enabled: boolean) => void;
}) {
  const { t } = useI18n();
  return <ul className="asb-native-telemetry-clients">
    {clients.map((client) => {
      const name = clientName(client.app);
      return <li key={client.app} className="asb-native-telemetry-client">
        <div>
          <strong>{name}</strong>
          {client.error && <p className="asb-field-error" role="alert">
            {t("usagePerformance.exportUnavailable")} · {client.error}
          </p>}
        </div>
        <Switch label={t("usagePerformance.clientExport", { client: name })} checked={client.configured}
          disabled={busy || Boolean(client.error) || (!listening && !client.configured)}
          onChange={(enabled) => onPreview(client.app, enabled)} />
      </li>;
    })}
  </ul>;
}

function ExportPreview({ pending, busy, error, onConfirm, onCancel }: {
  pending: PendingTelemetryExport; busy: boolean; error: string | null; onConfirm: () => void; onCancel: () => void;
}) {
  const { t } = useI18n();
  return <ConfirmSheet title={t("usagePerformance.clientExport", { client: clientName(pending.app) })}
    busy={busy} confirmLabel={t("usagePerformance.applyExport")} onConfirm={onConfirm} onCancel={onCancel}>
    <p>{t("usagePerformance.exportScope")}</p>
    <p><code>{pending.preview.file.preview.target}</code></p>
    {pending.preview.file.preview.warnings.map((warning) => <p className="asb-field-error" key={warning.key}>
      {localizedMessageText(warning, t)}</p>)}
    <DiffView changes={pending.preview.file.preview.changes} label={t("usagePerformance.exportChanges")} />
    {error && <p className="asb-field-error" role="alert">{error}</p>}
  </ConfirmSheet>;
}
