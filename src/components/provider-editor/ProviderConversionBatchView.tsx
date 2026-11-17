import type { ConversionBatch } from "../../app/provider-conversion-batch";
import { useI18n } from "../../i18n";
import { errorDiagnosticText, errorText, localizedMessageText } from "../../i18n/errors";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import "../../styles/base/provider-conversion.css";

export function ProviderConversionBatchView({ batch, busy, onSkip, onRetry, onEnd }: {
  batch: ConversionBatch; busy: boolean; onSkip: () => void; onRetry: () => void; onEnd: () => void;
}) {
  const { t } = useI18n();
  const source = batch.sourceApp === "codex" ? "Codex" : "Claude";
  const target = batch.sourceApp === "codex" ? "Claude" : "Codex";
  const complete = batch.phase === "complete";
  const waiting = batch.phase === "ready" || batch.phase === "preparing" || batch.phase === "refreshing";
  const locked = busy || waiting || batch.phase === "saving";
  const title = complete ? t("providers.conversion.complete") : t("providers.conversion.progress", {
    source, target, current: batch.index + 1, total: batch.sources.length,
  });
  const actions = <>
    {!complete && <Button variant="secondary" disabled={locked} onClick={onSkip}>{t("providers.conversion.skip")}</Button>}
    {batch.phase === "error" && <Button variant="primary" disabled={busy} onClick={onRetry}>{t("providers.conversion.retry")}</Button>}
    <Button variant={complete ? "primary" : "secondary"} disabled={locked} onClick={onEnd}>
      {t(complete ? "providers.conversion.done" : "providers.conversion.end")}
    </Button>
  </>;
  if (batch.phase === "editing" || batch.phase === "saving") return <section className="asb-banner asb-conversion-progress" aria-label={t("providers.conversion.summary")}>
    <div><strong aria-live="polite">{title}</strong><p className="asb-field-help">{t("providers.conversion.editHelp")}</p></div>
    <div className="asb-panel-actions">{actions}</div>
  </section>;
  const diagnostic = batch.phase === "error" ? errorDiagnosticText(batch.error, t) : null;
  return <AppDialog title={title} busy={busy || waiting} onClose={onEnd} footer={actions}>
    {complete ? <p role="status">{t("providers.conversion.result", { saved: batch.saved, skipped: batch.skipped })}</p> : <>
      <p><strong>{batch.sources[batch.index].name}</strong></p>
      {waiting && <p role="status">{t("providers.conversion.preparing")}</p>}
      {batch.phase === "blocked" && <>
        <p>{t("providers.conversion.unavailable")}</p>
        <p>{localizedMessageText(batch.reason, t)}</p>
      </>}
      {batch.phase === "error" && <div role="alert">
        <p>{errorText(batch.error, t)}</p>{diagnostic && <p className="asb-field-help">{diagnostic}</p>}
      </div>}
      <p className="asb-field-help">{t("providers.conversion.endHelp")}</p>
    </>}
  </AppDialog>;
}
