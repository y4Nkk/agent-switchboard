import { useEffect, useRef, useState } from "react";
import { applyCodexSessionProviderTransfer, previewCodexSessionProviderTransfer, readCodexSessionProviderTransfer,
  restoreCodexSessionProviderTransfer, type CodexSessionProviderOperation, type CodexSessionProviderPreview } from "../../api/sessions";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { FactList } from "../FactList";

function Summary({ plan }: { plan: CodexSessionProviderPreview }) {
  const { t } = useI18n();
  return <>
    <FactList rows={[
      { label: t("sessions.transfer.historyRoot"), value: <code>{plan.historyRoot}</code> },
      ...(plan.databasePath ? [{ label: t("sessions.transfer.databasePath"), value: <code>{plan.databasePath}</code> }] : []),
      { label: t("sessions.transfer.target"), value: <code>{plan.targetProvider}</code> },
      { label: t("sessions.transfer.sessions"), value: plan.sessions },
      { label: t("sessions.transfer.rows"), value: plan.databaseRows },
      { label: t("sessions.transfer.files"), value: plan.files },
      ...plan.providers.map((group) => ({ label: group.provider === null ? t("sessions.transfer.unspecified")
        : group.provider || t("sessions.transfer.emptyProvider"),
      value: t("sessions.transfer.group", { rows: group.databaseRows, files: group.files }) })),
    ]} />
    {plan.missingOriginals > 0 && <p className="asb-warn-text">{t("sessions.transfer.missing", { count: plan.missingOriginals })}</p>}
    {plan.externalFiles.length > 0 && <>
      <p className="asb-scope-note">{t("sessions.transfer.external", { count: plan.externalFiles.length })}</p>
      <FactList rows={[{ label: t("sessions.transfer.externalPaths"),
        value: <ul>{plan.externalFiles.map((path) => <li key={path}><code>{path}</code></li>)}</ul> }]} />
    </>}
  </>;
}

export function CodexSessionProviderTransfer({ onClose, onChanged }: { onClose: () => void; onChanged: () => void }) {
  const { t } = useI18n();
  const [plan, setPlan] = useState<CodexSessionProviderPreview | null>(null);
  const [latest, setLatest] = useState<CodexSessionProviderOperation | null>(null);
  const [mode, setMode] = useState<"transfer" | "restore">("transfer");
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useMessageState();
  const [statusError, setStatusError] = useMessageState();
  const lock = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    let current = true;
    void readCodexSessionProviderTransfer().then((value) => {
      if (!current) return;
      setLatest(value);
      if (value?.phase === "applying" || value?.phase === "restoring") setMode("restore");
    }).catch((caught: unknown) => { if (current) setError(caught); })
      .finally(() => { if (current) setBusy(false); });
    return () => { current = false; mounted.current = false; };
  }, [setError]);
  const run = async (action: "preview" | "apply" | "restore") => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null); setStatusError(null); setConfirmed(false);
    try {
      if (action === "preview") {
        const next = await previewCodexSessionProviderTransfer();
        if (mounted.current) setPlan(next);
      } else {
        let next: CodexSessionProviderOperation;
        if (action === "apply") { if (!plan) return; next = await applyCodexSessionProviderTransfer(plan.id, confirmed); }
        else { if (!latest) return; next = await restoreCodexSessionProviderTransfer(latest.preview.id, confirmed); }
        if (mounted.current && next) { setLatest(next); setPlan(null); setMode("transfer"); onChanged(); }
      }
    } catch (caught) {
      if (mounted.current) {
        setError(caught); setPlan(null);
        try {
          const next = await readCodexSessionProviderTransfer();
          if (mounted.current) { setLatest(next); if (next?.phase === "applying" || next?.phase === "restoring") setMode("restore"); }
        } catch (caughtStatus) { if (mounted.current) setStatusError(caughtStatus); }
      }
    } finally { lock.current = false; if (mounted.current) setBusy(false); }
  };
  return <AppDialog title={t("sessions.transfer.title")} busy={busy} onClose={onClose} footer={<>
    {mode === "transfer" ? <><Button variant="secondary" disabled={busy} onClick={() => void run("preview")}>{t("sessions.transfer.preview")}</Button>
      {plan && <Button variant="primary" disabled={busy || !confirmed || plan.sessions === 0} onClick={() => void run("apply")}>{t("sessions.transfer.apply")}</Button>}</>
      : <Button variant="primary" disabled={busy || !confirmed} onClick={() => void run("restore")}>{t("sessions.transfer.confirmRestore")}</Button>}
  </>}>
    <p className="asb-scope-note">{t("sessions.transfer.scope")}</p>
    <p className="asb-field-help">{t("sessions.transfer.policy")}</p>
    {busy && <p role="status">{t("sessions.transfer.working")}</p>}
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    {statusError && <p className="asb-warn-text" role="alert">{statusError}</p>}
    {mode === "transfer" && plan && <><Summary plan={plan} />
      {plan.sessions === 0 ? <p role="status">{t("sessions.transfer.noChanges")}</p>
        : <Checkbox checked={confirmed} disabled={busy} onChange={setConfirmed} label={t("sessions.transfer.confirmScope")} />}</>}
    {latest && <section aria-label={t("sessions.transfer.lastOperation")}>
      <p role="status">{t(`sessions.transfer.phase.${latest.phase}`)}</p>
      <FactList rows={[{ label: t("sessions.transfer.backup"), value: <code>{latest.backupDirectory}</code> }]} />
      {mode === "transfer" && latest.phase !== "restored" && <Button variant="secondary" disabled={busy}
        onClick={() => { setMode("restore"); setPlan(null); setConfirmed(false); }}>{t("sessions.transfer.restore")}</Button>}
      {mode === "restore" && <><Summary plan={latest.preview} />
        <Checkbox checked={confirmed} disabled={busy} onChange={setConfirmed} label={t("sessions.transfer.confirmRestoreScope")} /></>}
    </section>}
  </AppDialog>;
}
