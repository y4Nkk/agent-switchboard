import { useRef, useState } from "react";
import { cleanupSessions, previewSessionCleanup, type SessionCleanupOutcome,
  type SessionCleanupPlan } from "../../api/sessions";
import { useI18n } from "../../i18n";
import { useMessageState } from "../../i18n/use-message-state";
import { localizedMessageText } from "../../i18n/errors";
import type { LocalizedMessage } from "../../api/switching";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { DatePicker } from "../DatePicker";

function defaultCutoff() {
  const date = new Date(); date.setUTCDate(date.getUTCDate() - 90);
  return date.toISOString().slice(0, 10);
}

/** Bulk cleanup of preserved Claude sessions by cutoff date. The preview is
 * advisory only; execution recomputes the range from the same date. */
export function SessionPurgeDialog({ onClose, onChanged }: { onClose: () => void; onChanged: () => void }) {
  const { t, language } = useI18n();
  const [cutoff, setCutoff] = useState(defaultCutoff);
  const [plan, setPlan] = useState<SessionCleanupPlan | null>(null);
  const [outcome, setOutcome] = useState<SessionCleanupOutcome | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useMessageState();
  const locked = useRef(false);
  const run = async (phase: "preview" | "purge") => {
    if (locked.current) return;
    locked.current = true; setWorking(true); setError(null);
    try {
      if (phase === "preview") {
        setOutcome(null);
        setPlan(await previewSessionCleanup(cutoff));
      } else {
        const next = await cleanupSessions(cutoff);
        setOutcome(next); setPlan(null); onChanged();
      }
    } catch (caught) { setError(caught); }
    finally { locked.current = false; setWorking(false); }
  };
  const warnings = (list: LocalizedMessage[]) =>
    list.map((warning, index) => <p className="asb-warn-text" key={index}>{localizedMessageText(warning, t)}</p>);
  return <AppDialog title={t("sessions.purge.title")} busy={working} onClose={onClose}
    footer={plan === null ? <Button variant="secondary" disabled={working || !cutoff}
      onClick={() => void run("preview")}>{t("sessions.purge.preview")}</Button>
      : <Button variant="danger" disabled={working || plan.sessions === 0}
        onClick={() => void run("purge")}>{t("sessions.purge.confirm")}</Button>}>
    <label className="asb-cutoff-field">{t("sessions.purge.cutoff")}
      <DatePicker value={cutoff} disabled={working} ariaLabel={t("sessions.purge.cutoff")}
        onChange={(value) => { setCutoff(value); setPlan(null); }} />
    </label>
    <p className="asb-field-help">{t("sessions.purge.policy")}</p>
    {plan && <p className="asb-scope-note" role="status">
      {t("sessions.purge.review", { count: plan.sessions,
        size: `${(plan.bytes / 1024 / 1024).toLocaleString(language, { maximumFractionDigits: 1 })} MiB` })}</p>}
    {error && <p className="asb-warn-text" role="alert">{error}</p>}
    {outcome && <div role="status"><p>{t("sessions.purge.done", { count: outcome.removed })}</p>
      {warnings(outcome.warnings)}</div>}
  </AppDialog>;
}
