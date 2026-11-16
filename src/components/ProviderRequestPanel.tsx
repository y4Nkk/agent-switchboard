import type { ProviderDiagnosticKind, ProviderRequestTarget, ProviderRequestOutcome } from "../api/client";
import type { MessageKey } from "../i18n";
import { useI18n } from "../i18n";
import { protocolLabel } from "../lib/protocol";
import { Button } from "./Button";
import { ProviderDiagnosticDetails } from "./ProviderDiagnosticDetails";
import { Input } from "./Input";
import { ModelPicker } from "./ModelPicker";
import { Time } from "./Time";
import { ScrollArea } from "./ScrollArea";
import { CheckIcon, CloseIcon, RequestIcon } from "./icons";
import { useProviderRequest, type RequestView } from "./use-provider-request";

const OUTCOME_TITLES: Record<ProviderRequestOutcome, MessageKey> = {
  success: "providers.request.outcome.success",
  authenticationFailed: "providers.request.outcome.authFailed",
  rateLimited: "providers.request.outcome.rateLimited",
  httpError: "providers.request.outcome.httpError",
  networkError: "providers.request.outcome.networkError",
  timeout: "providers.request.outcome.timeout",
  invalidResponse: "providers.request.outcome.invalidResponse",
  cancelled: "providers.request.outcome.cancelled",
};

const DIAGNOSTIC_TITLES: Record<ProviderDiagnosticKind, MessageKey> = {
  dns: "providers.request.diag.dns",
  tls: "providers.request.diag.tls",
  network: "providers.request.diag.network",
  timeout: "providers.request.diag.timeout",
  websocketUnsupported: "providers.request.diag.websocketUnsupported",
  endpoint: "providers.request.diag.endpoint",
  authentication: "providers.request.diag.authentication",
  requestParameters: "providers.request.diag.requestParameters",
  modelNotFound: "providers.request.diag.modelNotFound",
  rateLimit: "providers.request.diag.rateLimit",
  upstream: "providers.request.diag.upstream",
  responseParse: "providers.request.diag.responseParse",
  streamParse: "providers.request.diag.streamParse",
};

const PHASE_TITLES: Record<RequestView["phase"], MessageKey> = {
  preparing: "providers.request.phase.preparing",
  ready: "providers.request.phase.ready",
  sending: "providers.request.phase.sending",
  cancelling: "providers.request.phase.cancelling",
  complete: "providers.request.phase.complete",
  failed: "providers.request.phase.failed",
  cancelled: "providers.request.phase.cancelled",
};

function RequestReceipt({ view }: { view: RequestView }) {
  const { t } = useI18n();
  const { phase, result, error } = view;
  const diagnostic = result?.diagnostic;
  const success = result?.outcome === "success";
  const cancelled = result?.outcome === "cancelled" || phase === "cancelled";
  const failure = phase === "failed" || (result !== null && !success && !cancelled);
  return (
    <div className="asb-request-receipt" data-phase={success ? "success" : failure ? "failure" : phase}
      role="status" aria-label={t("providers.request.receipt.aria")} aria-live="polite">
      <h4 className="asb-section-title">
        {success ? <CheckIcon /> : failure ? <CloseIcon /> : <RequestIcon />}
        {diagnostic ? t(DIAGNOSTIC_TITLES[diagnostic.kind]) : result ? t(OUTCOME_TITLES[result.outcome]) : t(PHASE_TITLES[phase])}
      </h4>
      {phase === "ready" && <p>{t("providers.request.note.ready")}</p>}
      {phase === "sending" && <p>{t("providers.request.note.sending")}</p>}
      {phase === "cancelling" && <p>{t("providers.request.note.cancelling")}</p>}
      {phase === "cancelled" && <p>{t("providers.request.note.cancelled")}</p>}
      {success && <ScrollArea className="asb-request-reply" aria-label={t("providers.request.receipt.aria")}>
        <blockquote>{result.reply}</blockquote>
      </ScrollArea>}
      {diagnostic ? <ProviderDiagnosticDetails diagnostic={diagnostic} /> : result?.error && <p>{result.error}</p>}
      {result && <>
        <p className="asb-request-metrics">
          {!diagnostic && result.status !== null && <>HTTP {result.status} · </>}{result.latencyMs} ms
          {result.model !== null && <>{t("providers.request.metrics.model", { model: result.model })}</>}
        </p>
        <p><Time iso={result.at} /></p>
      </>}
      {error && <p role="alert">{error}</p>}
    </div>
  );
}

export function ProviderRequestPanel({ target, name }: { target: ProviderRequestTarget; name: string }) {
  const { t } = useI18n();
  const request = useProviderRequest(target);
  const { view, model, busy } = request;
  const preparation = view.preparation;
  const preparing = view.phase === "preparing";
  // A listing in flight owns the model field: the stale list must not stay
  // selectable while the backend is resolving the current connection.
  const modelsLocked = busy || request.modelsBusy;
  return (
    <section className="asb-request-panel" aria-label={t("providers.request.aria", { name })}>
      {preparation && <dl className="asb-request-target">
        <div><dt>{t("providers.request.field.endpoint")}</dt><dd>{preparation.endpoint}</dd></div>
        <div><dt>{t("providers.label.apiFormat")}</dt><dd>{protocolLabel(t, preparation.upstreamProtocol)}</dd></div>
      </dl>}
      <div className="asb-request-body">
        <div className="asb-request-composer">
          <div className="asb-request-field">
            <span>{t("providers.request.field.model")}</span>
            <div className="asb-model-control">
              <Input code aria-label={t("providers.request.field.model")} value={model} placeholder={t("providers.request.field.modelPlaceholder")} disabled={modelsLocked || !preparation}
                onChange={(event) => request.setModel(event.target.value)} />
              {request.models && <ModelPicker models={request.models.map(({ id, ownedBy }) => ({ value: id, label: id, group: ownedBy }))} current={model} ariaLabel={t("providers.request.field.modelPickerAria")}
                disabled={modelsLocked || !preparation} onSelect={request.setModel} />}
              <div className="asb-model-actions">
                <Button variant="secondary" disabled={modelsLocked || !preparation} onClick={() => void request.fetchModels()}>
                  {request.modelsBusy ? t("providers.request.fetching") : t("providers.request.fetchModels")}
                </Button>
              </div>
            </div>
            {request.modelsError && <span className="asb-warn-text">{request.modelsError}</span>}
          </div>
          {preparation && <div className="asb-request-message"><span>{t("providers.request.field.prompt")}</span><p>{preparation.prompt}</p></div>}
          <p className="asb-request-note">{t(target.kind === "draft" ? "providers.request.note.draft" : "providers.request.note.saved")}</p>
        </div>
        <RequestReceipt view={view} />
      </div>
      <footer className="asb-request-footer">
        <p className="asb-request-note">{t("providers.request.note.cost")}</p>
        {!preparation && !preparing ? (
          <Button variant="primary" onClick={request.reload}>{t("providers.request.reload")}</Button>
        ) : busy ? (
          <Button variant="secondary" disabled={view.phase === "cancelling"} onClick={request.cancel}>
            <CloseIcon />{view.phase === "cancelling" ? t("providers.request.cancelling") : t("providers.request.cancel")}
          </Button>
        ) : (
          <Button variant="primary" disabled={preparing || request.modelsBusy || !preparation || !model.trim()} onClick={request.send}>
            <RequestIcon />{t("providers.request.send")}
          </Button>
        )}
      </footer>
    </section>
  );
}
