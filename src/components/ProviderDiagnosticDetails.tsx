import { useState } from "react";
import type { ProviderDiagnostic } from "../api/provider-request";
import { useI18n } from "../i18n";
import { CollapsibleSection } from "./Collapsible";
import { ScrollArea } from "./ScrollArea";

export function ProviderDiagnosticDetails({ diagnostic }: { diagnostic: ProviderDiagnostic }) {
  const { t } = useI18n();
  const [bodyOpen, setBodyOpen] = useState(false);
  return (
    <div className="asb-request-diagnostic">
      <p>{diagnostic.message}</p>
      <dl>
        <div><dt>{t("providers.label.requestUrl")}</dt><dd>{diagnostic.endpoint}</dd></div>
        <div><dt>{t("providers.request.detail.transport")}</dt><dd>{diagnostic.transport === "websocket" ? "WebSocket" : "HTTP/SSE"}</dd></div>
        <div><dt>{t("providers.request.detail.httpStatus")}</dt><dd>{diagnostic.status === null ? t("providers.request.detail.noHttpResponse") : `HTTP ${diagnostic.status}`}</dd></div>
        <div><dt>Request ID</dt><dd>{diagnostic.requestId ?? t("providers.request.detail.noRequestId")}</dd></div>
        {diagnostic.retryAfter !== null && <div>
          <dt>{t("providers.request.detail.retryAfter")}</dt><dd>{diagnostic.retryAfter}</dd>
        </div>}
      </dl>
      {diagnostic.body !== null && <CollapsibleSection quiet open={bodyOpen} onOpenChange={setBodyOpen}
        label={<>{t("providers.request.detail.upstreamBody")}{diagnostic.bodyTruncated ? t("providers.request.detail.bodyTruncated") : ""}</>}>
        <ScrollArea className="asb-request-diagnostic-body" aria-label={t("providers.request.detail.upstreamBody")}>
          <pre>{diagnostic.body || t("providers.request.detail.emptyBody")}</pre>
        </ScrollArea>
      </CollapsibleSection>}
    </div>
  );
}
