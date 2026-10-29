import { useEffect, useRef, useState } from "react";
import { cleanupBackups, previewBackupCleanup, type BackupCleanupPlan,
  type HistoryArchiveOutcome } from "../api/storage";
import { useI18n } from "../i18n";
import { useMessageState } from "../i18n/use-message-state";
import { localizedMessageText } from "../i18n/errors";
import type { LocalizedMessage } from "../api/switching";
import { Button } from "./Button";
import { ConfirmSheet } from "./ConfirmSheet";
import { DatePicker } from "./DatePicker";

function defaultCutoff() {
  const date = new Date(); date.setUTCDate(date.getUTCDate() - 90);
  return date.toISOString().slice(0, 10);
}

/** Bulk cleanup of recovery backups by cutoff date: whole backup groups are
 * exported to an archive outside application data before local copies are
 * deleted; each client's newest group is always retained. */
export function BackupCleanup({ busy, onCleaned }: { busy: boolean; onCleaned: () => void }) {
  const { t, language } = useI18n();
  const [cutoff, setCutoff] = useState(defaultCutoff);
  const [plan, setPlan] = useState<BackupCleanupPlan | null>(null);
  const [outcome, setOutcome] = useState<HistoryArchiveOutcome | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  const mounted = useRef(false);
  const generation = useRef(0);
  const locked = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current += 1; }; }, []);
  const run = async (phase: "preview" | "cleanup") => {
    if (locked.current) return;
    locked.current = true; setWorking(true); setError(null);
    try {
      if (phase === "preview") {
        const version = ++generation.current;
        const next = await previewBackupCleanup(cutoff);
        if (mounted.current && generation.current === version) setPlan(next);
      } else if (plan) {
        const next = await cleanupBackups(plan);
        if (mounted.current && next) { setOutcome(next); setPlan(null); onCleaned(); }
      }
    } catch (caught) { if (mounted.current) setError(caught); }
    finally { locked.current = false; if (mounted.current) setWorking(false); }
  };
  const warnings = (list: LocalizedMessage[]) =>
    list.map((warning, index) => <p className="asb-warn-text" key={index}>{localizedMessageText(warning, t)}</p>);
  const disabled = busy || working;
  return <>
    <h4 className="asb-group-title">{t("backup.cleanup.heading")}</h4>
    <label className="asb-cutoff-field">{t("backup.cleanup.cutoff")}
      <DatePicker value={cutoff} disabled={disabled} ariaLabel={t("backup.cleanup.cutoff")}
        onChange={(value) => { setCutoff(value); setPlan(null); }} />
    </label>
    <p className="asb-field-help">{t("backup.cleanup.policy")}</p>
    <Button variant="secondary" disabled={disabled || !cutoff}
      onClick={() => void run("preview")}>{t("backup.cleanup.preview")}</Button>
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    {outcome && <div role="status"><p>{t("backup.cleanup.done",
      { count: outcome.removed, path: outcome.path })}</p>
      {warnings(outcome.warnings)}</div>}
    {plan && <ConfirmSheet title={t("backup.cleanup.heading")} confirmLabel={t("backup.cleanup.archiveConfirm")}
      busy={working} confirmDisabled={disabled || plan.digest.records === 0}
      destructive onCancel={() => setPlan(null)} onConfirm={() => { void run("cleanup"); }}>
      <p>{t("backup.cleanup.review", { count: plan.digest.records, files: plan.files,
        size: `${(plan.bytes / 1024 / 1024).toLocaleString(language, { maximumFractionDigits: 1 })} MiB` })}</p>
    </ConfirmSheet>}
  </>;
}
