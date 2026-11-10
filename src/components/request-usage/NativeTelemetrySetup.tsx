import type { NativeTelemetrySetup as Setup, AppKind } from "../../api/client";
import { useState } from "react";
import { useI18n } from "../../i18n";
import { localizedMessageText } from "../../i18n/errors";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { CollapsibleSection } from "../Collapsible";
import { ConfirmSheet } from "../ConfirmSheet";
import { DiffView } from "../DiffView";
import { FactList } from "../FactList";
import { Table, type TableColumn } from "../Table";
import { Time } from "../Time";
import { useNativeTelemetry, type PendingTelemetryExport } from "./use-native-telemetry";

export function NativeTelemetrySetup({ active, onApplied }: { active: boolean; onApplied: () => void }) {
  const { t } = useI18n();
  const state = useNativeTelemetry(active, onApplied);
  const [open, setOpen] = useState(false);
  const receiver = state.data?.receiver;
  return <div className="asb-native-telemetry-setup">
    <p className="asb-request-usage-status" role="status">
      <span>{t("usagePerformance.receiver")}{" · "}
        {t(state.readError ? "usagePerformance.receiverUnavailable" : receiver
          ? receiver.listening ? "usagePerformance.listening" : "usagePerformance.notListening" : "usagePerformance.loading")}</span>
      {receiver && <code>{receiver.endpoint}</code>}
      {receiver?.lastExportAtMs !== null && receiver?.lastExportAtMs !== undefined && <span>
        {t("usagePerformance.lastExport")} <Time iso={new Date(receiver.lastExportAtMs).toISOString()} mode="relative" /></span>}
      {receiver && !receiver.listening && <Button size="compact" variant="secondary" disabled={state.busy} onClick={() => void state.retry()}>
        {t("usagePerformance.retryListener")}</Button>}
      {state.data?.clients.some((client) => !client.configured && !client.error) && !open && <Button
        size="compact" variant="ghost" onClick={() => setOpen(true)}>{t("usagePerformance.configureCollection")}</Button>}
    </p>
    {state.readError && <p className="asb-field-error" role="alert">{state.readError}</p>}
    {receiver?.error && <p className="asb-field-error" role="alert">{receiver.error}</p>}
    {state.error && !state.pending && <p className="asb-field-error" role="alert">{state.error}</p>}
    {state.applied && <p className="asb-scope-note" role="status">{t("usagePerformance.exportApplied")}</p>}
    <CollapsibleSection quiet open={open} onOpenChange={setOpen} label={t("usagePerformance.exportSettings")}>
      <p className="asb-scope-note">{t("usagePerformance.exportHelp")}</p>
      {receiver && <FactList rows={[
        { label: t("usagePerformance.acceptedRecords"), value: receiver.acceptedRecords },
        { label: t("usagePerformance.ignoredRecords"), value: receiver.ignoredRecords },
      ]} />}
      {state.data && <ExportClients clients={state.data.clients} busy={state.busy}
        listening={state.data.receiver.listening && !state.readError} onPreview={(app, enabled) => void state.preview(app, enabled)} />}
    </CollapsibleSection>
    {state.pending && <ExportPreview pending={state.pending} busy={state.busy} error={state.error}
      onConfirm={() => void state.commit()} onCancel={state.cancel} />}
  </div>;
}

function ExportClients({ clients, busy, listening, onPreview }: {
  clients: Setup["clients"]; busy: boolean; listening: boolean;
  onPreview: (app: AppKind, enabled: boolean) => void;
}) {
  const { t } = useI18n();
  const columns: TableColumn<Setup["clients"][number]>[] = [
    { key: "app", header: t("usage.column.client"), render: (client) => clientName(client.app) },
    { key: "status", header: t("usagePerformance.exportStatus"), render: (client) => <>
      {t(client.configured ? "usagePerformance.exportConfigured" : "usagePerformance.exportNotConfigured")}
      {client.error && <span className="asb-request-usage-caption asb-field-error">{client.error}</span>}</> },
    { key: "action", header: t("usagePerformance.exportSettings"), render: (client) => <Button
      size="compact" variant="secondary" disabled={busy || Boolean(client.error) || (!listening && !client.configured)}
      onClick={() => onPreview(client.app, !client.configured)}>
      {t(client.configured ? "usagePerformance.disableExport" : "usagePerformance.enableExport")}</Button> },
  ];
  return <Table columns={columns} rows={clients} rowKey={(client) => client.app} ariaLabel={t("usagePerformance.exportSettings")} />;
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
